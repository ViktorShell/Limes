use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc},
};

use crate::runtime::lambda::*;
use anyhow::Context;
use crc32fast::Hasher;
use tokio::sync::RwLock;
use wasmtime::{component::Component, Config, Engine};

pub type UserId = String;
pub type ModuleId = u32;

#[derive(Debug, Default)]
pub struct UserModules {
    wasm_modules: Arc<RwLock<HashMap<ModuleId, ModuleHandler>>>,
    loaded_functions: Arc<RwLock<HashMap<FunctionId, FunctionHandler>>>,
}

impl UserModules {
    pub async fn insert_module(
        &self,
        engine: &Engine,
        key: ModuleId,
        bytes: &[u8],
    ) -> anyhow::Result<u32> {
        // Create the module
        let module_handler = ModuleHandler(Arc::new(
            Component::from_binary(engine, bytes)
                .context("UserModule: Unable to register the module")?,
        ));

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

    pub async fn contains_module(&self, module_id: &ModuleId) -> bool {
        self.wasm_modules.read().await.contains_key(module_id)
    }

    pub async fn remove_module(&self, module_id: &ModuleId) {
        // WARNING: Check if removing the module while the function is loaded create errors
        self.wasm_modules.write().await.remove(module_id);
    }
}

pub struct ModuleHandler(Arc<Component>);

impl std::fmt::Debug for ModuleHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModuleHandler")
    }
}

#[derive(Debug)]
pub struct Runtime {
    memory_size: usize,
    max_allocatable_functions: usize,
    currently_allocated_functions: Arc<AtomicUsize>,
    wasm_engine: Arc<Engine>,
    users: Arc<RwLock<HashMap<UserId, UserModules>>>,
}

impl Runtime {
    #[allow(warnings)]
    pub fn new() -> RuntimeBuilder {
        RuntimeBuilder {
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

    pub async fn remove_user(&self, user_id: &UserId) -> bool {
        if self.users.read().await.contains_key(user_id) {
            self.users.write().await.remove(user_id);
            return true;
        }
        false
    }

    pub async fn register_module(
        &self,
        user_id: &UserId,
        bytes: &[u8],
    ) -> anyhow::Result<ModuleId> {
        let user_guard = self.users.read().await;

        self.check_if_user_exist(user_id).await?;

        // Insert the module in the UserModules
        // NOTE: Secure unwrap due to UserModules::default()
        let user_module = user_guard.get(user_id).unwrap();

        let key_hash = user_module.get_hash(bytes).await;
        let module_id: ModuleId = user_module
            .insert_module(&self.wasm_engine, key_hash, bytes)
            .await
            .context("Runtime: Failed to load the Component from the bytes")?;
        Ok(module_id)
    }

    pub async fn remove_module(
        &self,
        user_id: &UserId,
        module_id: &ModuleId,
    ) -> anyhow::Result<()> {
        let user_guard = self.users.read().await;

        self.check_if_user_exist(user_id).await?;

        // Get UserModules
        let user_module = user_guard.get(user_id).unwrap();
        if user_module.contains_module(module_id).await {
            user_module.remove_module(module_id).await;
        }
        Ok(())
    }

    async fn check_if_user_exist(&self, user_id: &UserId) -> anyhow::Result<bool> {
        let user_guard = self.users.read().await;
        if !user_guard.contains_key(user_id) {
            return Err(anyhow::anyhow!(
                "Runtime: Didn't fine the user with id: {user_id}"
            ));
        };
        Ok(true)
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
    memory_size: Option<usize>,
    max_functions: Option<usize>,
}

impl RuntimeBuilder {
    pub fn set_memory_size(&mut self, memory_size: usize) -> &mut Self {
        self.memory_size = Some(memory_size);
        self
    }

    pub fn set_max_functions(&mut self, max_functions: usize) -> &mut Self {
        self.max_functions = Some(max_functions);
        self
    }

    /// Will build the wasmtime engine and configure the Runtime
    pub async fn build(&self) -> anyhow::Result<Arc<Runtime>> {
        let engine = Engine::new(
            Config::new()
                .async_support(true)
                .wasm_component_model(true)
                .cranelift_opt_level(wasmtime::OptLevel::SpeedAndSize),
        )
        .with_context(|| "Runtime: Failed to build the Wasmtime Engine")?;

        // Check if is already setted
        Ok(Arc::new(Runtime {
            memory_size: self.memory_size.unwrap_or(1024 * 1024 * 10),
            max_allocatable_functions: self.max_functions.unwrap_or(100),
            currently_allocated_functions: Arc::new(AtomicUsize::new(0)),
            wasm_engine: Arc::new(engine),
            users: Arc::new(RwLock::new(HashMap::new())),
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Read;
    use std::path::Path;
    use std::path::PathBuf;

    // NOTE: Directory where the wasm functions are located
    static WASM_RESOURCES: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/resources/lamda_tests/wasm_compiled"
    );

    /// Test User registration and deletion
    #[tokio::test]
    async fn user_registartion_deletion() {
        let rt = Runtime::new().build().await.unwrap();

        let user_id_1 = rt.register_user().await.unwrap();
        let user_id_2 = rt.register_user().await.unwrap();
        let user_id_3 = rt.register_user().await.unwrap();
        assert!(!user_id_1.is_empty());
        assert!(!user_id_2.is_empty());
        assert!(!user_id_3.is_empty());
        rt.remove_user(&user_id_1).await;
        assert_eq!(rt.users.read().await.len(), 2);
    }

    /// Test Module Registration and Deletion
    #[tokio::test]
    async fn module_registration_and_deletion() {
        let rt = Runtime::new().build().await.unwrap();

        // User Id's
        let user_id_one = rt.register_user().await.unwrap();
        let user_id_two = rt.register_user().await.unwrap();

        // Preload Modules as Bytes
        let wasm_mod_1_path =
            PathBuf::from(&format!("{}/{}", WASM_RESOURCES, "stop_infinite_loop.wasm"));
        let wasm_mod_2_path = PathBuf::from(&format!(
            "{}/{}",
            WASM_RESOURCES, "multiple_function_exec.wasm"
        ));
        let wasm_mod_3_path = PathBuf::from(&format!(
            "{}/{}",
            WASM_RESOURCES, "exec_rust_lambda_function.wasm"
        ));

        let wasm_mod_1 = load_from_file(&wasm_mod_1_path);
        let wasm_mod_2 = load_from_file(&wasm_mod_2_path);
        let wasm_mod_3 = load_from_file(&wasm_mod_3_path);

        // Register some modules
        // User 1
        let module_id_1_1 = rt.register_module(&user_id_one, &wasm_mod_1).await.unwrap();
        let module_id_1_2 = rt.register_module(&user_id_one, &wasm_mod_1).await.unwrap();
        // User 2
        let module_id_2_3 = rt.register_module(&user_id_two, &wasm_mod_3).await.unwrap();

        dbg!(module_id_1_2);
        dbg!(&rt.users);
    }

    fn load_from_file(path: &Path) -> Vec<u8> {
        let file = File::open(path).unwrap();
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).unwrap();
        buffer
    }
}
