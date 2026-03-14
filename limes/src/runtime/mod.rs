use std::{
    collections::HashMap,
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, OnceLock,
    },
};

use crate::runtime::lambda::*;
use crate::runtime::runtime_builder::*;
use crate::runtime::types::*;
use crate::runtime::user_module::*;

use anyhow::Context;
use nanoid::nanoid;
use tokio::sync::RwLock;
use wasmtime::{Config, Engine};

pub mod lambda;
pub mod runtime_builder;
pub mod types;
pub mod user_module;

#[derive(Debug)]
pub struct Runtime {
    pub memory_size: usize, // FIX: Must implement the increment using Atomics
    pub max_allocatable_functions: usize, // FIX: Must implement
    pub currently_allocated_functions: Arc<AtomicUsize>, // FIX: Must implement
    pub wasm_engine: Arc<Engine>,
    pub users: Arc<RwLock<HashMap<UserId, UserModules>>>,
}

static RUNTIME_REF: OnceLock<Arc<Runtime>> = OnceLock::new();

impl Runtime {
    pub fn new() -> RuntimeBuilder {
        RuntimeBuilder {
            memory_size: Some(1024 * 1024 * 100),
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
        }
        Ok(true)
    }

    pub async fn load_function(
        &self,
        user_id: &UserId,
        module_id: &ModuleId,
        function_memory_size: usize,
        tap_ip: Ipv4Addr,
        function_name: String,
        function_input_description: String,
        description: String,
    ) -> anyhow::Result<FunctionId> {
        // Check if already exists
        let users = self.users.read().await;
        let user = users.get(user_id).ok_or(anyhow::anyhow!(
            "Runtime: No user found with the following id"
        ))?;

        // Get the module
        let module = user.get_modules(module_id).await.ok_or(anyhow::anyhow!(
            "Runtime: Error with the registered component"
        ))?;

        // Function memory
        let memory = self.memory_size - function_memory_size;
        if memory == 0 {
            return Err(anyhow::anyhow!(
                "Runtime: Not enought memory to allocate the current function"
            ));
        }

        // FIX: must fix
        // self.memory_size = self.memory_size - function_memory_size;

        // Build the function & FunctionId
        let func_id = nanoid!();
        let func_handl = FunctionHandler::new(
            module.component,
            function_memory_size,
            tap_ip,
            function_name,
            function_input_description,
            description,
            user_id.clone(),
            func_id.clone(),
        )
        .await?;

        // Register the function by hash
        let mut loaded_funcs = user.loaded_functions.write().await;
        loaded_funcs.insert(func_id.clone(), Arc::new(func_handl));

        // Increase counter FIX: must fix
        let mut current = self.currently_allocated_functions.load(Ordering::SeqCst);
        self.currently_allocated_functions
            .store(current + 1, Ordering::SeqCst);

        Ok(func_id)
    }

    pub async fn unload_function(
        &self,
        user_id: &UserId,
        function_id: &FunctionId,
    ) -> anyhow::Result<()> {
        // Check if already exists
        let users = self.users.read().await;
        let user = users.get(user_id).ok_or(anyhow::anyhow!(
            "Runtime: No user found with the following id"
        ))?;

        // Get the module
        let mut module = user.loaded_functions.write().await;
        module.remove_entry(function_id);
        Ok(())
    }

    pub async fn exec_function(
        &self,
        user_id: &UserId,
        function_id: &FunctionId,
        args: &str,
    ) -> anyhow::Result<String> {
        // Search for the user and function
        let users = self.users.read().await;
        let user = users.get(user_id).ok_or(anyhow::anyhow!(
            "Runtime: No user found with the following id"
        ))?;

        let func_map = user.loaded_functions.read().await;
        let functions = func_map.get(function_id).ok_or(anyhow::anyhow!(
            "Runtime: Function not found with id: {}",
            function_id
        ));
        let func = functions?;
        let result = func.lambda.run(args).await?;
        Ok(result)
    }

    pub async fn stop_function(
        &self,
        user_id: &UserId,
        function_id: &FunctionId,
    ) -> anyhow::Result<()> {
        // Search for the user and function
        let users = self.users.read().await;
        let user = users.get(user_id).ok_or(anyhow::anyhow!(
            "Runtime: No user found with the following id"
        ))?;

        let func_map = user.loaded_functions.read().await;
        let functions = func_map.get(function_id).ok_or(anyhow::anyhow!(
            "Runtime: Function not found with id: {}",
            function_id
        ));
        let func = functions?;
        func.lambda.stop().await
    }

    pub fn get_runtime_ref() -> anyhow::Result<Arc<Runtime>> {
        let rt_ref = RUNTIME_REF.get().context("Runtime: Not initialized")?;
        Ok(rt_ref.clone())
    }

    pub async fn get_user_functions(
        &self,
        user_id: &UserId,
    ) -> anyhow::Result<Vec<Arc<FunctionHandler>>> {
        let user_guard = self.users.read().await;
        let user_modules = user_guard
            .get(user_id)
            .ok_or(anyhow::anyhow!("Runtime: User not found"))?;

        let user_func_guard = user_modules.loaded_functions.read().await;
        let func_arr: Vec<Arc<FunctionHandler>> = user_func_guard.values().cloned().collect();

        Ok(func_arr)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Read;
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
        let wasm_mod_1 = load_from_file("stop_infinite_loop.wasm");
        let wasm_mod_2 = load_from_file("multiple_function_exec.wasm");
        let wasm_mod_3 = load_from_file("exec_rust_lambda_function.wasm");

        // Register some modules
        // User 1
        let module_id_1_1 = rt
            .register_module(&user_id_one, &wasm_mod_1)
            .await
            .unwrap_or(0);
        let module_id_1_2 = rt
            .register_module(&user_id_one, &wasm_mod_2)
            .await
            .unwrap_or(0);
        // User 2
        let module_id_2_3 = rt
            .register_module(&user_id_two, &wasm_mod_3)
            .await
            .unwrap_or(0);

        assert!(module_id_1_1 != 0);
        assert!(module_id_1_2 != 0);
        assert!(module_id_2_3 != 0);
    }

    #[tokio::test]
    async fn load_functions_and_execute() {
        let rt = Runtime::new().build().await.unwrap();

        // Defines two users
        let user_one_id = rt.register_user().await.unwrap();
        let user_two_id = rt.register_user().await.unwrap();

        // Load modules
        let module_op_a_b = load_from_file("op_a_b.wasm");
        let module_sorter = load_from_file("multiple_function_exec.wasm");
        let module_infinite_loop = load_from_file("stop_infinite_loop.wasm");

        // Register modules for users_one
        let user_one_module_sorter_id = rt
            .register_module(&user_one_id, &module_sorter)
            .await
            .unwrap();
        let user_one_module_op_a_b_id = rt
            .register_module(&user_one_id, &module_op_a_b)
            .await
            .unwrap();

        // Register modules for user_two
        let user_two_module_op_a_b_id = rt
            .register_module(&user_two_id, &module_op_a_b)
            .await
            .unwrap();
        let user_two_module_infinite_loop_id = rt
            .register_module(&user_two_id, &module_infinite_loop)
            .await
            .unwrap();

        // Load functions for user one
        let tap_ip = Ipv4Addr::new(127, 0, 0, 1);

        let user_one_op_a_b_func_id = rt
            .load_function(
                &user_one_id,
                &user_one_module_op_a_b_id,
                1024 * 1024 * 2,
                tap_ip,
                "sum".to_string(),
                "some input".to_string(),
                "execute the sum between two numbers a and b".to_string(),
            )
            .await
            .unwrap();

        let user_one_sorter_func_id = rt
            .load_function(
                &user_one_id,
                &user_one_module_sorter_id,
                1024 * 1024 * 2,
                tap_ip,
                "sort".to_string(),
                "some input".to_string(),
                "sort a set of item in the input form like i1,i2,i3,i4".to_string(),
            )
            .await
            .unwrap();

        // Load functions for user_two
        let user_two_op_a_b_func_id = rt
            .load_function(
                &user_two_id,
                &user_two_module_op_a_b_id,
                1024 * 1024 * 2,
                tap_ip,
                "sum".to_string(),
                "some input".to_string(),
                "execute the sum between two numbers a and b".to_string(),
            )
            .await
            .unwrap();

        let user_two_infinite_loop_func = rt
            .load_function(
                &user_two_id,
                &user_two_module_infinite_loop_id,
                1024 * 1024 * 2,
                tap_ip,
                "loop".to_string(),
                "some input".to_string(),
                "is an infinite loop, used for the testing".to_string(),
            )
            .await
            .unwrap();

        // Exec functions in parallel
        // 1. Spawna a task to stop the infinite loop function
        tokio::spawn({
            let rt_clone = rt.clone();
            let uid = user_two_id.clone();
            let fid = user_two_infinite_loop_func.clone();

            async move {
                // Diamo il tempo alla funzione Wasm di iniziare l'esecuzione
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                rt_clone.stop_function(&uid, &fid).await
            }
        });

        // Join on parallel execution
        let (user_one_op_r, user_one_sort_r, user_two_op_r, user_two_inf_r) = tokio::join!(
            rt.exec_function(&user_one_id, &user_one_op_a_b_func_id, "15 + 16"),
            rt.exec_function(&user_one_id, &user_one_sorter_func_id, "f,e,c,d,a,x"),
            rt.exec_function(&user_two_id, &user_two_op_a_b_func_id, "15 / 5"),
            rt.exec_function(&user_two_id, &user_two_infinite_loop_func, "")
        );

        assert_eq!(user_one_op_r.unwrap(), "31");
        assert_eq!(user_one_sort_r.unwrap(), "[a,c,d,e,f,x]");
        assert_eq!(user_two_op_r.unwrap(), "3");
        assert!(user_two_inf_r.is_err());
    }

    fn load_from_file(wasm_name: &str) -> Vec<u8> {
        let path = PathBuf::from(&format!("{}/{}", WASM_RESOURCES, wasm_name));
        let file = File::open(path).unwrap();
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).unwrap();
        buffer
    }
}
