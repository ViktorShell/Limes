use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, OnceLock,
    },
};

use log::*;
use nanoid::nanoid;
use thiserror::Error;
use tokio::sync::RwLock;
use wasmtime::Engine;

use crate::runtime::lambda::FunctionHandler;
use crate::runtime::runtime_builder::RuntimeBuilder;
use crate::runtime::types::{FunctionId, ModuleId, UserId};
use crate::runtime::user_module::UserModules;

pub mod lambda;
pub mod runtime_builder;
pub mod types;
pub mod user_module;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Module not found: {0}")]
    ModuleNotFound(ModuleId),

    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error(
        "Insufficient memory: requested {requested} bytes but only {available} bytes available"
    )]
    InsufficientMemory { requested: usize, available: usize },

    #[error("Runtime not initialized — call Runtime::new().build() first")]
    NotInitialized,

    #[error("Engine configuration error: {0}")]
    EngineConfig(String),
}

// Singleton
static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

#[derive(Debug)]
pub struct Runtime {
    /// Total memory budget shared across all loaded functions.
    pub memory_size: usize,
    /// Maximum number of concurrently loaded functions.
    pub max_allocatable_functions: usize,
    pub currently_allocated_functions: Arc<AtomicUsize>,
    pub wasm_engine: Arc<Engine>,
    pub users: Arc<RwLock<HashMap<UserId, UserModules>>>,
}

impl Runtime {
    /// Returns a builder with sensible defaults (100 MiB, 100 functions).
    pub fn runtime_builder() -> RuntimeBuilder {
        RuntimeBuilder::default()
    }

    /// Returns a clone of the global singleton `Arc<Runtime>`.
    pub fn get_runtime_ref() -> Result<Arc<Runtime>, RuntimeError> {
        RUNTIME.get().cloned().ok_or(RuntimeError::NotInitialized)
    }

    pub async fn register_user(&self) -> anyhow::Result<UserId> {
        let mut user_guard = self.users.write().await;
        let user_id = uuid::Uuid::new_v4().to_string();
        user_guard.insert(user_id.clone(), UserModules::default());
        info!("User registered with id: {user_id}");
        Ok(user_id)
    }

    pub async fn remove_user(&self, user_id: &UserId) -> bool {
        let mut user_guard = self.users.write().await;
        let removed = user_guard.remove(user_id).is_some();
        if removed {
            info!("User removed with id: {user_id}");
        } else {
            warn!("Attempted to remove unknown user: {user_id}");
        }
        removed
    }

    pub async fn register_module(
        &self,
        user_id: &UserId,
        bytes: &[u8],
    ) -> anyhow::Result<ModuleId> {
        let mut users = self.users.write().await;
        let user_modules = users
            .get_mut(user_id)
            .ok_or_else(|| RuntimeError::UserNotFound(user_id.clone()))?;

        let key = user_modules.compute_hash(bytes);
        let module_id = user_modules
            .insert_module(&self.wasm_engine, key, bytes)
            .await;

        let module_id =
            module_id.map_err(|_| anyhow::anyhow!("Error while registering the module id"))?;

        info!(
            r#"
                Wasm module registered:
                >> user_id: {user_id}
                >> module_id: {module_id}"#
        );

        Ok(module_id)
    }

    pub async fn remove_module(
        &self,
        user_id: &UserId,
        module_id: &ModuleId,
    ) -> anyhow::Result<()> {
        let mut users = self.users.write().await;
        let user_modules = users
            .get_mut(user_id)
            .ok_or_else(|| RuntimeError::UserNotFound(user_id.clone()))?;

        if user_modules.contains_module(module_id).await {
            user_modules.remove_module(module_id).await;
            info!(
                r#"
                Wasm module remove:
                >> user_id: {user_id}
                >> module_id: {module_id}"#
            );
        } else {
            warn!(
                r#"
                Attempted to remove unknow module:
                >> user_id: {user_id}
                >> module_id: {module_id}"#
            );
        }
        Ok(())
    }

    pub async fn load_function(
        &self,
        user_id: &UserId,
        module_id: &ModuleId,
        function_memory_size: usize,
        function_name: String,
        description: String,
        function_input_description_json: String,
    ) -> anyhow::Result<FunctionId> {
        let available = self.memory_size;
        if function_memory_size > available {
            return Err(RuntimeError::InsufficientMemory {
                requested: function_memory_size,
                available,
            }
            .into());
        }

        let mut users_guard = self.users.write().await;
        let user_modules = users_guard
            .get_mut(user_id)
            .ok_or_else(|| RuntimeError::UserNotFound(user_id.clone()))?;

        let module_handler = user_modules
            .get_module(module_id)
            .await
            .ok_or(RuntimeError::ModuleNotFound(*module_id))?;

        let function_id = nanoid!();
        let handler = FunctionHandler::new(
            module_handler.component,
            function_memory_size,
            function_name.clone(),
            function_input_description,
            description,
            user_id.clone(),
            function_id.clone(),
        )
        .await?;

        let _ = user_modules
            .loaded_functions
            .insert(function_id.clone(), Arc::new(handler));

        self.currently_allocated_functions
            .fetch_add(1, Ordering::SeqCst);

        info!(
            r#"
Function loaded:
>> user_id: {user_id}
>> function_id: {function_id}
>> function_name: {function_name}
        "#
        );

        Ok(function_id)
    }

    pub async fn unload_function(
        &self,
        user_id: &UserId,
        function_id: &FunctionId,
    ) -> anyhow::Result<()> {
        let mut users = self.users.write().await;
        let user_modules = users
            .get_mut(user_id)
            .ok_or_else(|| RuntimeError::UserNotFound(user_id.clone()))?;

        user_modules.loaded_functions.remove(function_id);

        self.currently_allocated_functions
            .fetch_sub(1, Ordering::SeqCst);

        info!(
            r#"
Function unloaded:
>> user_id: {user_id}
>> function_id: {function_id}
"#
        );

        Ok(())
    }

    pub async fn exec_function(
        &self,
        user_id: &UserId,
        function_id: &FunctionId,
        args: &str,
    ) -> anyhow::Result<String> {
        debug!(
            r#"
Executing function:
    user_id: {user_id}
    function_id: {function_id}
        "#
        );

        let users = self.users.read().await;
        let user_modules = users
            .get(user_id)
            .ok_or_else(|| RuntimeError::UserNotFound(user_id.clone()))?;

        let functions = &user_modules.loaded_functions;
        let handler = functions
            .get(function_id)
            .ok_or_else(|| RuntimeError::FunctionNotFound(function_id.clone()))?;

        let result = handler.lambda.run(args).await?;
        info!(
            r#"
Function executed successfully:
>> user_id: {user_id}
>> function_id: {function_id}
>> result: {result}
        "#
        );

        Ok(result)
    }

    pub async fn stop_function(
        &self,
        user_id: &UserId,
        function_id: &FunctionId,
    ) -> anyhow::Result<()> {
        info!(
            r#"
Function interruption signal:
>> user_id: {user_id}
>> function_id: {function_id}
        "#
        );
        let users = self.users.read().await;
        let user_modules = users
            .get(user_id)
            .ok_or_else(|| RuntimeError::UserNotFound(user_id.clone()))?;

        let functions = &user_modules.loaded_functions;
        let handler = functions
            .get(function_id)
            .ok_or_else(|| RuntimeError::FunctionNotFound(function_id.clone()))?;

        handler.lambda.stop().await
    }

    pub async fn get_user_functions(
        &self,
        user_id: &UserId,
    ) -> anyhow::Result<Vec<Arc<FunctionHandler>>> {
        let user_guard = self.users.read().await;
        let user_modules = user_guard
            .get(user_id)
            .ok_or(anyhow::anyhow!("Runtime: User not found"))?;

        let user_func_guard = &user_modules.loaded_functions;
        let func_arr: Vec<Arc<FunctionHandler>> = user_func_guard.values().cloned().collect();

        Ok(func_arr)
    }

    pub(crate) fn set_global(rt: Arc<Runtime>) {
        // Ignore the error — in tests multiple runtimes may be built
        let _ = RUNTIME.set(rt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    static WASM_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/resources/lamda_tests/wasm_compiled"
    );

    fn load_bytes(name: &str) -> Vec<u8> {
        let path = PathBuf::from(format!("{WASM_DIR}/{name}"));
        std::fs::read(path).expect("wasm file not found")
    }

    #[tokio::test]
    async fn user_registration_and_removal() {
        let rt = Runtime::runtime_builder().build().await.unwrap();
        let u1 = rt.register_user().await.unwrap();
        let u2 = rt.register_user().await.unwrap();
        assert!(!u1.is_empty());
        assert!(!u2.is_empty());
        assert!(rt.remove_user(&u1).await);
        assert_eq!(rt.users.read().await.len(), 1);
        // Removing non-existent user returns false
        assert!(!rt.remove_user(&u1).await);
    }

    #[tokio::test]
    async fn module_registration_and_removal() {
        let rt = Runtime::runtime_builder().build().await.unwrap();
        let user = rt.register_user().await.unwrap();
        let bytes = load_bytes("exec_rust_lambda_function.wasm");
        let mid = rt.register_module(&user, &bytes).await.unwrap();
        assert_ne!(mid, 0);
        rt.remove_module(&user, &mid).await.unwrap();
    }

    #[tokio::test]
    async fn load_and_exec_function() {
        let rt = Runtime::runtime_builder().build().await.unwrap();
        let user = rt.register_user().await.unwrap();

        let sorter_bytes = load_bytes("sorter.wasm");
        let op_bytes = load_bytes("op_a_b.wasm");

        let mid_sort = rt.register_module(&user, &sorter_bytes).await.unwrap();
        let mid_op = rt.register_module(&user, &op_bytes).await.unwrap();

        let fid_sort = rt
            .load_function(
                &user,
                &mid_sort,
                1024 * 1024 * 2,
                "sort".into(),
                "comma-separated list".into(),
                "sort items".into(),
            )
            .await
            .unwrap();

        let fid_op = rt
            .load_function(
                &user,
                &mid_op,
                1024 * 1024 * 2,
                "calc".into(),
                "expression".into(),
                "math".into(),
            )
            .await
            .unwrap();

        let (r_sort, r_op) = tokio::join!(
            rt.exec_function(&user, &fid_sort, "f,e,c,d,a,x"),
            rt.exec_function(&user, &fid_op, "15 + 16")
        );

        assert_eq!(r_sort.unwrap(), "[a,c,d,e,f,x]");
        assert_eq!(r_op.unwrap(), "31");
    }

    #[tokio::test]
    async fn stop_infinite_loop_via_runtime() {
        let rt = Runtime::runtime_builder().build().await.unwrap();
        let user = rt.register_user().await.unwrap();
        let bytes = load_bytes("stop_infinite_loop.wasm");
        let mid = rt.register_module(&user, &bytes).await.unwrap();
        let fid = rt
            .load_function(
                &user,
                &mid,
                1024 * 1024 * 2,
                "loop".into(),
                "".into(),
                "infinite loop".into(),
            )
            .await
            .unwrap();

        tokio::spawn({
            let rt2 = rt.clone();
            let u = user.clone();
            let f = fid.clone();
            async move {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                rt2.stop_function(&u, &f).await.unwrap();
            }
        });

        let result = rt.exec_function(&user, &fid, "").await;
        assert!(result.is_err());
    }
}
