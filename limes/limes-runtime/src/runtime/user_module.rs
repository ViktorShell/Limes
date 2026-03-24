use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use crc32fast::Hasher;
use wasmtime::{component::Component, Engine};

use super::lambda::FunctionHandler;
use super::types::{FunctionId, ModuleId};

// ─────────────────────────────────────────────────────────────────────────────
//  ModuleHandler — thin wrapper around a compiled Wasm component
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ModuleHandler {
    pub component: Arc<Component>,
}

impl std::fmt::Debug for ModuleHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ModuleHandler")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  UserModules — all modules and loaded functions for a single user
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct UserModules {
    pub wasm_modules: HashMap<ModuleId, ModuleHandler>,
    pub loaded_functions: HashMap<FunctionId, Arc<FunctionHandler>>,
}

impl UserModules {
    /// Compile `bytes` into a Wasm component and store it under `key`.
    /// Returns the key on success.
    pub async fn insert_module(
        &mut self,
        engine: &Engine,
        key: ModuleId,
        bytes: &[u8],
    ) -> Result<ModuleId> {
        let component = Component::from_binary(engine, bytes)
            .map_err(|e| anyhow::anyhow!("UserModules: failed to compile component: {e}"))?;

        self.wasm_modules.insert(
            key,
            ModuleHandler {
                component: Arc::new(component),
            },
        );

        Ok(key)
    }

    /// Compute a CRC-32 fingerprint of the bytes, used as the `ModuleId`.
    pub fn compute_hash(&self, bytes: &[u8]) -> ModuleId {
        let mut hasher = Hasher::new();
        hasher.update(bytes);
        hasher.finalize()
    }

    pub async fn get_module(&self, module_id: &ModuleId) -> Option<ModuleHandler> {
        self.wasm_modules.get(module_id).cloned()
    }

    pub async fn contains_module(&self, module_id: &ModuleId) -> bool {
        self.wasm_modules.contains_key(module_id)
    }

    pub async fn remove_module(&mut self, module_id: &ModuleId) {
        self.wasm_modules.remove(module_id);
    }
}
