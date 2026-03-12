use super::lambda::*;
use super::types::*;
use anyhow::Context;
use crc32fast::Hasher;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use wasmtime::{component::Component, Engine};

#[derive(Debug, Default)]
pub struct UserModules {
    pub wasm_modules: Arc<RwLock<HashMap<ModuleId, ModuleHandler>>>,
    pub loaded_functions: Arc<RwLock<HashMap<FunctionId, Arc<FunctionHandler>>>>,
}

impl UserModules {
    pub async fn insert_module(
        &self,
        engine: &Engine,
        key: ModuleId,
        bytes: &[u8],
    ) -> anyhow::Result<u32> {
        // Create the module
        let module_handler = ModuleHandler {
            component: Arc::new(
                Component::from_binary(engine, bytes)
                    .context("UserModule: Unable to register the module")?,
            ),
        };

        // Insert the module
        (*self.wasm_modules)
            .write()
            .await
            .insert(key, module_handler);
        Ok(key)
    }

    pub async fn get_hash(&self, bytes: &[u8]) -> ModuleId {
        let mut hasher = Hasher::new();
        hasher.update(bytes);
        hasher.finalize()
    }

    pub async fn get_modules(&self, module_id: &ModuleId) -> Option<ModuleHandler> {
        self.wasm_modules.read().await.get(module_id).cloned()
    }

    pub async fn contains_module(&self, module_id: &ModuleId) -> bool {
        self.wasm_modules.read().await.contains_key(module_id)
    }

    pub async fn remove_module(&self, module_id: &ModuleId) {
        self.wasm_modules.write().await.remove(module_id);
    }
}

#[derive(Clone)]
pub struct ModuleHandler {
    pub component: Arc<Component>,
}

impl std::fmt::Debug for ModuleHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModuleHandler")
    }
}
