import requests
import sys
import os
from typing import List, Dict


# ==========================================
# BLOCCO 1: Client API (Invariato)
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
            headers={"Content-Type": "application/octet-stream"},
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


# ==========================================
# BLOCCO 2: Logica Multi-File, Multi-Utente
# ==========================================
def run_multi_test(client: RuntimeAPIClient, wasm_paths: List[str]):
    print("🚀 Inizio test: Utenti Multipli, MODULI Multipli, Funzioni e Input...")

    # --- 1. Pre-caricamento di TUTTI i file WASM ---
    # Facciamo questo prima di creare utenti per evitare di sporcare il server se manca un file
    wasm_modules = {}
    print("\n📂 Caricamento file WASM locali...")
    for path in wasm_paths:
        try:
            with open(path, "rb") as f:
                wasm_modules[path] = f.read()
            filename = os.path.basename(path)
            print(
                f"   ✅ Caricato in RAM: {filename} ({len(wasm_modules[path])} bytes)"
            )
        except FileNotFoundError:
            print(f"❌ Errore fatale: File WASM non trovato in '{path}'")
            sys.exit(1)

    if not wasm_modules:
        print("⚠️ Nessun file WASM fornito. Test interrotto.")
        return

    # --- Configurazione del Test ---
    NUM_USERS = 2
    FUNCTIONS_PER_MODULE = 2  # Quante funzioni istanziare per OGNI modulo WASM
    INPUTS_TO_TEST = ["Input Veloce", "Input Complesso"]

    active_users = []

    try:
        # --- Ciclo Utenti ---
        for u_idx in range(NUM_USERS):
            user_id = client.create_user()
            active_users.append(user_id)
            print(f"\n👤 [Utente {u_idx + 1}/{NUM_USERS}] Creato: {user_id}")

            # --- Ciclo Moduli WASM ---
            for path, wasm_bytes in wasm_modules.items():
                filename = os.path.basename(path)
                module_id = client.register_module(user_id, wasm_bytes)
                print(f"   📦 Modulo '{filename}' registrato con ID: {module_id}")

                # --- Ciclo Funzioni per questo Modulo ---
                for f_idx in range(FUNCTIONS_PER_MODULE):
                    func_config = {
                        "function_memory_size": 1024 * 1024 * 2,  # 2 MB
                        "tap_ip": f"192.168.{u_idx}.{f_idx + 1}",  # IP simulato
                        "function_name": f"func_{filename}_{f_idx}",
                        "function_input_description": "Input test",
                        "description": f"Istanza {f_idx} del modulo {filename}",
                    }

                    func_id = client.load_function(user_id, module_id, func_config)
                    print(f"      ⚙️ Funzione {f_idx + 1} istanziata: {func_id}")

                    # --- Ciclo Input per questa Funzione ---
                    for inp_idx, input_data in enumerate(INPUTS_TO_TEST):
                        payload = f"[{func_config['function_name']}] {input_data}"
                        try:
                            result = client.exec_function(user_id, func_id, payload)
                            print(
                                f"         ✅ Exec input {inp_idx + 1}: {result.strip()}"
                            )
                        except requests.exceptions.HTTPError as e:
                            print(f"         ❌ Exec input {inp_idx + 1} FALLITO: {e}")

    finally:
        # ==========================================
        # BLOCCO 3: Pulizia Finale
        # ==========================================
        print("\n🧹 Avvio pulizia delle risorse...")
        for user_id in active_users:
            success = client.remove_user(user_id)
            status = "✅" if success else "❌"
            print(f"   {status} Rimozione utente {user_id}")

    print("\n🎉 Test completato!")


# ==========================================
# PUNTO DI INGRESSO
# ==========================================
if __name__ == "__main__":
    BASE_URL = "http://localhost:3000"

    # Inserisci qui la lista di tutti i file WASM che vuoi testare
    # Puoi aggiungere quanti percorsi vuoi.
    LISTA_WASM = [
        "percorso/al/tuo/primo_file.wasm",
        "percorso/al/tuo/secondo_file.wasm",
        # "moduli/math_module.wasm",
        # "moduli/string_manipulator.wasm"
    ]

    api_client = RuntimeAPIClient(BASE_URL)

    try:
        run_multi_test(api_client, LISTA_WASM)
    except requests.exceptions.ConnectionError:
        print(
            f"❌ Errore: Impossibile connettersi a {BASE_URL}. Il server Axum è acceso?"
        )
