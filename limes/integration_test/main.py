"""
Limes Integration Test
======================
Tests the full flow:
  1. Start the Limes server (requires: ollama serve + ollama run llama3.2:3b)
  2. Register a user.
  3. Load the `calculator` and `agent_executor` Wasm modules.
  4. Load functions from each module.
  5. Execute a math query through the agent (which uses the calculator as a tool).
  6. Clean up all allocated resources.

Run with:
    source .venv/bin/activate
    python main.py
"""

from __future__ import annotations

import sys
import textwrap
from pathlib import Path
from typing import Any

import requests


# ─────────────────────────────────────────────────────────────────────────────
#  Server URL and Wasm paths
# ─────────────────────────────────────────────────────────────────────────────

BASE_URL = "http://localhost:50500"
WASM_DIR = Path(__file__).parent / "wasm_files"


# ─────────────────────────────────────────────────────────────────────────────
#  RuntimeAPIClient
# ─────────────────────────────────────────────────────────────────────────────


class RuntimeAPIClient:
    """Thin Python wrapper around the Limes REST API."""

    def __init__(self, base_url: str) -> None:
        self.base_url = base_url.rstrip("/")
        self.session = requests.Session()

    # ── Users ────────────────────────────────────────────────────────────────

    def create_user(self) -> str:
        """Register a new user; returns the assigned user_id."""
        res = self.session.post(f"{self.base_url}/users")
        res.raise_for_status()
        return res.json()["user_id"]

    def remove_user(self, user_id: str) -> bool:
        res = self.session.delete(f"{self.base_url}/users/{user_id}")
        return res.status_code == 200

    # ── Modules ──────────────────────────────────────────────────────────────

    def register_module(self, user_id: str, wasm_bytes: bytes) -> int:
        """Upload a Wasm binary; returns the module_id (u32)."""
        res = self.session.post(
            f"{self.base_url}/users/{user_id}/modules",
            data=wasm_bytes,
            headers={
                "Content-Type": "application/octet-stream",
                "Content-Length": str(len(wasm_bytes)),
            },
        )
        res.raise_for_status()
        return res.json()["module_id"]

    # ── Functions ────────────────────────────────────────────────────────────

    def load_function(
        self,
        user_id: str,
        module_id: int,
        *,
        function_memory_size: int,
        function_name: str,
        function_input_description: str,
        description: str,
    ) -> str:
        """Instantiate a function from a module; returns the function_id."""
        payload: dict[str, Any] = {
            "function_memory_size": function_memory_size,
            "function_name": function_name,
            "function_input_description": function_input_description,
            "description": description,
        }
        res = self.session.post(
            f"{self.base_url}/users/{user_id}/modules/{module_id}/functions",
            json=payload,
        )
        res.raise_for_status()
        return res.json()["function_id"]

    def exec_function(self, user_id: str, function_id: str, args: str) -> str:
        """Execute a loaded function and return the string result."""
        res = self.session.post(
            f"{self.base_url}/users/{user_id}/functions/{function_id}/exec",
            data=args,
            headers={"Content-Type": "text/plain"},
        )
        res.raise_for_status()
        return res.json()["result"]


# ─────────────────────────────────────────────────────────────────────────────
#  Helpers
# ─────────────────────────────────────────────────────────────────────────────


def load_wasm(name: str) -> bytes:
    path = WASM_DIR / name
    if not path.exists():
        print(f"[ERROR] Wasm file not found: {path}")
        sys.exit(1)
    data = path.read_bytes()
    print(f"  Loaded {name} ({len(data):,} bytes)")
    return data


def separator(title: str = "") -> None:
    width = 60
    if title:
        print(f"\n{'─' * 4} {title} {'─' * (width - len(title) - 6)}")
    else:
        print("─" * width)


# ─────────────────────────────────────────────────────────────────────────────
#  Main test flow
# ─────────────────────────────────────────────────────────────────────────────


def main() -> None:
    client = RuntimeAPIClient(BASE_URL)

    # ── Step 1: Load Wasm binaries ───────────────────────────────────────────
    separator("Loading Wasm files")
    agent_bytes = load_wasm("agent_executor.wasm")
    calculator_bytes = load_wasm("calculator.wasm")

    # ── Step 2: Register user ────────────────────────────────────────────────
    separator("User")
    user_id = client.create_user()
    print(f"  user_id = {user_id}")

    try:
        # ── Step 3: Register modules ─────────────────────────────────────────
        separator("Modules")
        calculator_module_id = client.register_module(user_id, calculator_bytes)
        print(f"  calculator    module_id   = {calculator_module_id}")
        agent_module_id = client.register_module(user_id, agent_bytes)
        print(f"  agent_executor module_id   = {agent_module_id}")

        # ── Step 4: Load functions ───────────────────────────────────────────
        separator("Functions")
        calculator_fn_id = client.load_function(
            user_id,
            calculator_module_id,
            function_memory_size=1024 * 1024 * 2,
            function_name="calculator",
            function_input_description=('{"expression": "string"}'),
            description="Evaluates a simple arithmetic expression and returns the result as a string. An example of expression is rappresented as `5 + 2 - 3 * 4 / 1`",
        )
        print(f"  calculator function_id = {calculator_fn_id}")

        agent_fn_id = client.load_function(
            user_id,
            agent_module_id,
            function_memory_size=1024 * 1024 * 2,
            function_name="agent",
            function_input_description='{"ask_agent": "string"}',
            description=(
                "An LLM agent able to answer to simple questions"
                "to answer questions or execute tasks."
            ),
        )
        print(f"  agent function_id     = {agent_fn_id}")

        # ── Step 5: Smoke-test the calculator directly ───────────────────────
        separator("Calculator smoke test")
        expr = '{"expression": "5 * 5 + 5 / 3 * 1"}'
        calc_result = client.exec_function(user_id, calculator_fn_id, expr)
        print(f"  {expr} = {calc_result}")
        assert calc_result == "26", f"Unexpected result: {calc_result!r}"
        print("  ✓ Calculator assertion passed")

        # ── Step 6: Agent query that exercises the calculator tool ────────────
        separator("Agent integration test")
        query = "Use the calculator tool to solve the following expression '5 * 5 - 2 + 7 - 16 / 4 + 2'"
        print(f"  Query: {query}")
        answer = client.exec_function(user_id, agent_fn_id, query)
        print(f"  Agent answer:\n{textwrap.indent(answer, '    ')}")

    finally:
        # ── Step 7: Cleanup ──────────────────────────────────────────────────
        separator("Cleanup")
        removed = client.remove_user(user_id)
        print(f"  User {user_id} removed: {removed}")

    separator()
    print("Integration test completed successfully.")


if __name__ == "__main__":
    main()
