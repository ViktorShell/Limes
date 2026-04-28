use super::lambda::FunctionHandler;
use super::types::{FunctionId, ModuleId, UserId};
use anyhow::Result;
use crc32fast::Hasher;
use nanoid::nanoid;
use std::{collections::HashMap, sync::Arc};
use wasmtime::{component::Component, Engine};

// ─────────────────────────────────────────────────────────────────────────────
//  ModuleHandler — thin wrapper around a compiled Wasm component
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ModuleHandler {
    pub component: Arc<Component>,
    pub function_name: String,
    pub function_description: String,
    pub function_input_json: Option<String>,
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
        function_name: String,
        function_description: String,
        function_input_json: Option<String>,
    ) -> Result<ModuleId> {
        let component = Component::from_binary(engine, bytes)
            .map_err(|e| anyhow::anyhow!("UserModules: failed to compile component: {e}"))?;

        self.wasm_modules.insert(
            key,
            ModuleHandler {
                component: Arc::new(component),
                function_name,
                function_description,
                function_input_json,
            },
        );

        Ok(key)
    }

    pub async fn insert_function(
        &mut self,
        user_id: &UserId,
        module_id: &ModuleId,
    ) -> Result<FunctionId> {
        let module = match self.wasm_modules.get(module_id) {
            Some(m) => m,
            _ => return Err(anyhow::anyhow!("No module found with id: {module_id}")),
        };

        let function_id: FunctionId = nanoid!();
        let f_handler = FunctionHandler::new(
            module.component.clone(),
            user_id.into(),
            function_id.clone(),
            module.function_name.clone(),
            module.function_description.clone(),
            module.function_input_json.clone(),
        )
        .await?;

        let _ = self
            .loaded_functions
            .insert(function_id.clone(), Arc::new(f_handler));
        Ok(function_id)
    }

    pub async fn get_function(&self, function_id: &FunctionId) -> Result<&FunctionHandler> {
        let func_handl = match self.loaded_functions.get(function_id) {
            Some(f) => f,
            _ => return Err(anyhow::anyhow!("No function found with id: {function_id}")),
        };

        Ok(func_handl)
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
