import requests
import sys
from typing import Dict


# ==========================================
#  Client API
# ==========================================
class RuntimeAPIClient:
    def __init__(self, base_url: str):
        self.base_url = base_url

    def create_user(self) -> str:
        res = requests.post(f"{self.base_url}/users")
        res.raise_for_status()
        return res.json()["user_id"]

    def remove_user(self, user_id: str) -> bool:
        res = requests.delete(f"{self.base_url}/users/{user_id}")
        return res.status_code == 200

    def register_module(self, user_id: str, wasm_bytes: bytes) -> str:
        res = requests.post(
            f"{self.base_url}/users/{user_id}/modules",
            data=wasm_bytes,
            headers={
                "Content-Type": "application/octet-stream",
                "Content-Length": str(len(wasm_bytes)),
            },
        )
        res.raise_for_status()
        return res.json()["module_id"]

    def load_function(self, user_id: str, module_id: str, config: Dict) -> str:
        res = requests.post(
            f"{self.base_url}/users/{user_id}/modules/{module_id}/functions",
            json=config,
        )
        res.raise_for_status()
        return res.json()["function_id"]

    def exec_function(self, user_id: str, function_id: str, args: str) -> str:
        res = requests.post(
            f"{self.base_url}/users/{user_id}/functions/{function_id}/exec",
            data=args,
            headers={"Content-Type": "text/plain"},
        )
        res.raise_for_status()
        return res.json()["result"]


def load_wasm_files(wasm_path: str):
    print("-> Loading WASM files")
    try:
        with open(wasm_path, "rb") as f:
            wasm_bytes = f.read()
            print(f"\n -> Loaded file: {wasm_path}")
    except FileNotFoundError:
        print(f"-> File not found: {wasm_path}")
        sys.exit(1)
    return wasm_bytes


# ==========================================
# Main Test
# ==========================================
if __name__ == "__main__":
    BASE_URL = "http://localhost:50500"
    BASE_WASM_FOLDER = (
        "/home/viktor/Desktop/agentic_limes/limes/limes/integration_test/wasm_files/"
    )

    api_client = RuntimeAPIClient(BASE_URL)

    # Load WASM files
    wasm_agent_bytes = load_wasm_files(BASE_WASM_FOLDER + "agent_executor.wasm")
    wasm_calculator_bytes = load_wasm_files(BASE_WASM_FOLDER + "calculator.wasm")

    # Get user id
    user_id = api_client.create_user()
    print(f"-> Created user with id: {user_id}")

    # Load modules
    agent_module_id = api_client.register_module(user_id, wasm_agent_bytes)
    print(f"-> Loaded agent module with id: {agent_module_id}")
    calculator_module_id = api_client.register_module(user_id, wasm_calculator_bytes)
    print(f"-> Loaded calculator module with id: {calculator_module_id}")

    # Load functions
    agent_func_config = {
        "function_memory_size": 1024 * 1024 * 2,
        "tap_ip": "127.0.0.1",
        "function_name": "agent",
        "function_input_description": "input field of this function is a string with the question/task for the agent",
        "description": "This function allow the interaction with an agent which can answer to questions or execute tasks",
    }
    agent_function_id = api_client.load_function(
        user_id, agent_module_id, agent_func_config
    )
    print(f"-> Loaded agent function with id: {agent_function_id}")

    calculator_function_id = {
        "function_memory_size": 1024 * 1024 * 2,
        "tap_ip": "127.0.0.1",
        "function_name": "calculator",
        "function_input_description": 'the input must be formatted as list of integer numbers with the operator, like the example in the quotes "num * num - num + num"',
        "description": "This function is a calculator for a simple math expressions",
    }
    calculator_function_id = api_client.load_function(
        user_id, calculator_module_id, calculator_function_id
    )
    print(f"-> Loaded calculator function with id: {calculator_function_id}")

    # Exec a query
    query = "Can you use the calculator tool and give me the result of the following expression: 5 * 5 - 2 + 7 - 16 / 4 + 2"
    answer = api_client.exec_function(user_id, agent_function_id, query)
    print(f"ANSWER: {answer}")

    # Unload allocated resources
