use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc, OnceLock},
};

use crate::runtime::lambda::*;
use anyhow::Context;
use tokio::sync::RwLock;
use wasmtime::{component::Component, Config, Engine};

// NOTE: Used to initialized only one time the Runtime
static SINGLETON_RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub type UserId = String;
pub type ModuleId = String;

#[derive(Default)]
pub struct UserModules {
    wasm_modules: Arc<HashMap<ModuleId, ModuleHandler>>,
    loaded_functions: Arc<HashMap<FunctionId, FunctionHandler>>,
}

impl UserModules {
    pub fn insert_module(&mut self, bytes: &[u8]) -> bool {
        todo!()
    }

    pub fn contains_module(&self, module_id: ModuleId) -> bool {
        todo!()
    }

    pub fn remove_module(&mut self, module_id: ModuleId) -> bool {
        todo!()
    }
}

pub struct ModuleHandler(Arc<Component>);

pub struct Runtime {
    vcpus: usize,
    memory_size: usize,
    max_allocatable_functions: usize,
    currently_allocated_functions: Arc<AtomicUsize>,
    wasm_engine: Arc<Engine>,
    users: Arc<RwLock<HashMap<UserId, UserModules>>>,
}

impl Runtime {
    pub fn new() -> RuntimeBuilder {
        RuntimeBuilder {
            vcpus: Some(1),
            memory_size: Some(1024 * 1024 * 10),
            max_functions: Some(100),
        }
    }

    pub async fn register_user(&self) -> anyhow::Result<String> {
        let user_id: String = uuid::Uuid::new_v4().to_string();
        let user_module = UserModules::default();
        self.users
            .write()
            .await
            .insert(user_id.clone(), user_module);
        Ok(user_id)
    }

    pub async fn remove_user(&self, user_id: UserId) -> anyhow::Result<()> {
        if self.users.read().await.contains_key(&user_id) {
            self.users.write().await.remove(&user_id);
        }
        Ok(())
    }

    pub async fn register_module(&self, user_id: UserId, bytes: &[u8]) -> anyhow::Result<()> {
        // Check if user is registered
        if !self.users.read().await.contains_key(&user_id) {
            return Err(anyhow::anyhow!(
                "Runtime: Trying to register a module to a not registere user"
            ));
        }

        // Check if module is already registered
        if self.users.read().await.

        // Create the module_handler
        let engine = &*self.wasm_engine;
        let wasm_binary = Arc::new(
            Component::from_binary(engine, bytes)
                .context("Runtime: The binary file could not be loaded")?,
        );

        let module_handler = ModuleHandler(wasm_binary.clone());

        let user_module = self
            .users
            .write()
            .await
            .get_mut(&user_id)
            .context("Runtime: Could not read the UserModules")?;

        user_module.wasm_modules.Ok(())
    }

    pub async fn remove_module(&self) -> anyhow::Result<()> {
        todo!();
        Ok(())
    }

    pub async fn load_function(&self) -> anyhow::Result<()> {
        todo!();
        Ok(())
    }

    pub async fn exec_function(&self) -> anyhow::Result<()> {
        todo!();
        Ok(())
    }

    pub async fn unload_function(&self) -> anyhow::Result<()> {
        todo!();
        Ok(())
    }
}

pub struct RuntimeBuilder {
    vcpus: Option<usize>,
    memory_size: Option<usize>,
    max_functions: Option<usize>,
}

impl RuntimeBuilder {
    pub fn set_vcpus(&mut self, vcpus: usize) -> &mut Self {
        self.vcpus = Some(vcpus);
        self
    }

    pub fn set_memory_size(&mut self, memory_size: usize) -> &mut Self {
        self.memory_size = Some(memory_size);
        self
    }

    pub fn set_max_functions(&mut self, max_functions: usize) -> &mut Self {
        self.max_functions = Some(max_functions);
        self
    }

    /// Will build the wasmtime engine and configure the Runtime
    pub fn build(&self) -> anyhow::Result<&'static Runtime> {
        let engine = Engine::new(
            Config::new()
                .async_support(true)
                .wasm_component_model(true)
                .cranelift_opt_level(wasmtime::OptLevel::SpeedAndSize),
        )
        .with_context(|| "Failed to build the Wasmtime Engine")?;

        SINGLETON_RUNTIME
            .set(Runtime {
                vcpus: self.vcpus.unwrap_or(1),
                memory_size: self.memory_size.unwrap_or(1024 * 1024 * 10),
                max_allocatable_functions: self.max_functions.unwrap_or(100),
                currently_allocated_functions: Arc::new(AtomicUsize::new(0)),
                wasm_engine: Arc::new(engine),
                users: Arc::new(RwLock::new(HashMap::new())),
            })
            .map_err(|_| anyhow::anyhow!("Failed to Initialize the Runtime Singleton"))?;

        SINGLETON_RUNTIME
            .get()
            .with_context(|| "The Runtime was not initialized")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Test User registration and deletion
    #[tokio::test]
    async fn user_registartion_deletion() {
        let rt = Runtime::new().build().unwrap();
        let user_id_1 = rt.register_user().await.unwrap();
        let user_id_2 = rt.register_user().await.unwrap();
        let user_id_3 = rt.register_user().await.unwrap();
        assert!(!user_id_1.is_empty());
        assert!(!user_id_2.is_empty());
        assert!(!user_id_3.is_empty());
        rt.remove_user(user_id_1).await.unwrap();
        assert_eq!(rt.users.read().await.len(), 2);
    }
}
