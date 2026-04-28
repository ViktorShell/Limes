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
import base64

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

    def register_module(
        self,
        user_id: str,
        wasm_b64: str,
        function_name,
        function_description,
        function_input_json,
    ) -> int:
        """Upload a Wasm binary; returns the module_id (u32)."""
        payload: dict[str, Any] = {
            "wasm_base64": wasm_b64,
            "function_name": function_name,
            "function_description": function_description,
            "function_input_json": function_input_json,
        }

        res = self.session.post(
            f"{self.base_url}/users/{user_id}/modules",
            headers={
                "Content-Type": "application/json",
            },
            json=payload,
        )
        res.raise_for_status()
        return res.json()["module_id"]

    # ── Functions ────────────────────────────────────────────────────────────

    def load_function(
        self,
        user_id: str,
        module_id: int,
    ) -> str:
        """Instantiate a function from a module; returns the function_id."""
        res = self.session.post(
            f"{self.base_url}/users/{user_id}/modules/{module_id}/functions",
        )
        res.raise_for_status()
        return res.json()["function_id"]

    def exec_function(self, user_id: str, function_id: str, args: str) -> str:
        """Execute a loaded function and return the string result."""
        args_json: dict[str, Any] = {"arguments": args}
        res = self.session.post(
            f"{self.base_url}/users/{user_id}/functions/{function_id}/exec",
            headers={"Content-Type": "application/json"},
            json=args_json,
        )
        res.raise_for_status()
        return res.json()["result"]


# --- Helpers ---


def load_wasm_as_b64(name: str) -> str:
    path = WASM_DIR / name
    if not path.exists():
        print(f"[ERROR] Wasm file not found: {path}")
        sys.exit(1)
    data = path.read_bytes()
    data_b64 = base64.b64encode(data).decode("utf-8")
    print(f"  Loaded {name} ({len(data):,} bytes)")
    return data_b64


def separator(title: str = "") -> None:
    width = 60
    if title:
        print(f"\n{'─' * 4} {title} {'─' * (width - len(title) - 6)}")
    else:
        print("─" * width)


# --- Main test flow ---


def main() -> None:
    client = RuntimeAPIClient(BASE_URL)

    # ── Step 1: Load Wasm binaries ──
    separator("Loading Wasm files")
    agent_b64 = load_wasm_as_b64("agent_executor.wasm")
    calculator_b64 = load_wasm_as_b64("calculator.wasm")

    # ── Step 2: Register user ───
    separator("User")
    user_id = client.create_user()
    print(f"  user_id = {user_id}")

    try:
        # ── Step 3: Register modules ───
        separator("Modules")
        calculator_module_id = client.register_module(
            user_id,
            calculator_b64,
            "calculator",
            "Solve calculation problems in the form like 5 + 3 - 12 / 2",
            '{"required": ["expression"], "properties": { "expression": { "type": "string", "description": "An expression in form like 5 - 2 / 3 * 25"}}}',
        )
        print(f"  calculator    module_id   = {calculator_module_id}")

        agent_module_id = client.register_module(
            user_id,
            agent_b64,
            "llm_agent",
            "Is an LLM Agent able to solve the asked tasks and answer to questions, it have access to the local tools",
            '{"required": ["task"], "properties": {"task": {"type": "string", "description": "Any type of task or question"}}}',
        )
        print(f"  agent_executor module_id   = {agent_module_id}")

        # ── Step 4: Load functions ───
        separator("Functions")
        calculator_fn_id = client.load_function(
            user_id,
            calculator_module_id,
        )
        print(f"  calculator function_id = {calculator_fn_id}")

        agent_fn_id = client.load_function(
            user_id,
            agent_module_id,
        )
        print(f"  agent function_id     = {agent_fn_id}")

        # ── Step 5: Smoke-test the calculator directly ───
        separator("Calculator smoke test")
        expr = '{"expression": "5 * 5 - 2 + 7 - 16 + 2"}'
        calc_result = client.exec_function(user_id, calculator_fn_id, expr)
        print(f"  {expr} = {calc_result}")
        assert calc_result == '{"content": 16}', f"Unexpected result: {calc_result!r}"
        print("  ✓ Calculator assertion passed")

        # ── Step 6: Agent query that exercises the calculator tool ───
        separator("Agent integration test")
        query = '{"task": "Can you solve the following expression using the calculator tool -> 5 * 5 - 2 + 7 - 16 + 2"}'
        print(f"  Query: {query}")
        answer = client.exec_function(user_id, agent_fn_id, query)
        print(f"  Agent answer:\n{textwrap.indent(answer, '    ')}")

    finally:
        # ── Step 7: Cleanup ───
        separator("Cleanup")
        removed = client.remove_user(user_id)
        print(f"  User {user_id} removed: {removed}")

    separator()
    print("Integration test completed successfully.")


if __name__ == "__main__":
    main()
