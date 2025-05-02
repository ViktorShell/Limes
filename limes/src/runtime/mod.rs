use crc32fast::Hasher;
use dashmap::DashMap;
use lambda::Lambda;
use lambda_error::LambdaError;
use nanoid::nanoid;
use runtime_error::RuntimeError;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::RwLock;
use wasmtime::component::Component;
use wasmtime::Config;
use wasmtime::Engine;
use wasmtime::OptLevel;

pub mod lambda;
pub mod lambda_error;
pub mod runtime_error;

pub struct RuntimeBuilder {
    vcpus: Option<usize>,
    memory: Option<usize>,
    max_functions: Option<usize>,
    currently_allocated_functions: Option<usize>,
}

impl RuntimeBuilder {
    pub fn set_cpus(&mut self, vcpus: usize) -> &mut Self {
        self.vcpus = Some(vcpus);
        self
    }

    pub fn set_total_memory_size(&mut self, memory_size: usize) -> &mut Self {
        self.memory = Some(memory_size);
        self
    }

    pub fn set_max_functions_number(&mut self, size: usize) -> &mut Self {
        self.max_functions = Some(size);
        self
    }

    pub fn build(&self) -> Result<Runtime, RuntimeError> {
        let mut engines_config = Config::new();
        engines_config
            .async_support(true)
            .wasm_component_model(true)
            .cranelift_opt_level(OptLevel::SpeedAndSize);
        let engines: Vec<Arc<Engine>> = self.gen_engines(self.vcpus.unwrap(), &engines_config)?;

        Ok(Runtime {
            vcpus: self.vcpus.unwrap(),
            memory: self.memory.unwrap(),
            max_functions: self.max_functions.unwrap(),
            currently_allocated_functions: Arc::new(RwLock::new(
                self.currently_allocated_functions.unwrap(),
            )),
            engines: Arc::new(engines),
            engine_rotatory_index: Arc::new(RwLock::new(0)),
            modules: Arc::new(DashMap::new()),
            functions: Arc::new(DashMap::new()),
        })
    }

    fn gen_engines(
        &self,
        num: usize,
        config: &wasmtime::Config,
    ) -> Result<Vec<Arc<Engine>>, RuntimeError> {
        let mut engines = Vec::new();
        for _ in 0..num {
            let engine = Arc::new(Engine::new(config).map_err(|_| RuntimeError::EngineInitError)?);
            engines.push(engine);
        }
        Ok(engines)
    }
}

#[derive(PartialEq)]
pub enum FunctionHandlerStatus {
    Ready,
    Running,
    Stopped,
}

pub struct FunctionHandler {
    lambda: Lambda,
    status: FunctionHandlerStatus,
}

pub struct ModuleHandler {
    component: Arc<Component>,
    hash: u32,
}

type ModuleID = String;
type FunctionID = String;

pub struct Runtime {
    vcpus: usize,
    memory: usize,
    max_functions: usize,
    currently_allocated_functions: Arc<RwLock<usize>>,
    engines: Arc<Vec<Arc<Engine>>>,
    // Temporary solution for engine deployment, need a HeapMin for a good queue
    engine_rotatory_index: Arc<RwLock<usize>>,
    modules: Arc<DashMap<ModuleID, Arc<ModuleHandler>>>,
    functions: Arc<DashMap<FunctionID, Arc<RwLock<FunctionHandler>>>>,
}

impl Runtime {
    // Costruct the Runtime
    pub fn new() -> RuntimeBuilder {
        RuntimeBuilder {
            vcpus: Some(1),
            memory: Some(1024 * 1024 * 2 * 100), // Instance for 100 instance of 2Mb each
            max_functions: Some(100),
            currently_allocated_functions: Some(0),
        }
    }

    // Get the code in byteform, try to compile it and save it a sqa db and make it ready for
    // deployment, return an uuid of the module
    // NOTE: Add the following
    // - Register to DB
    // - Logger
    pub async fn register_module(&self, bytes: Vec<u8>) -> Result<ModuleID, RuntimeError> {
        let engine = self.get_engine().await;
        let hash = self.gen_module_hash(&bytes);
        // FIX: Should check on local and db if already present
        // using the hash otherwise create a module and register it.
        // for the seek of time I will not check the presence of the module in memory.
        let module_id = nanoid!(20, &nanoid::alphabet::SAFE);

        // Create component
        let component = Arc::new(
            wasmtime::component::Component::from_binary(&engine, &bytes)
                .map_err(|_| RuntimeError::ComponentBuildError)?,
        );
        self.modules.insert(
            module_id.clone(),
            Arc::new(ModuleHandler { component, hash }),
        );

        Ok(module_id)
    }

    fn gen_module_hash(&self, bytes: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(bytes);
        hasher.finalize()
    }

    // Remove a registered module
    // NOTE: Add the following
    // - Remove from DB
    // - Logger
    pub async fn remove_module(&self, id: ModuleID) -> Result<(), RuntimeError> {
        if self.modules.contains_key(&id) {
            self.modules.remove(&id);
            return Ok(());
        }
        Err(RuntimeError::ModuleNotRegistered)
    }

    // Initialize the function
    // Generate one
    // return a function_id
    pub async fn init_function(
        &self,
        id: ModuleID,
        tap_ip: Ipv4Addr,
    ) -> Result<FunctionID, RuntimeError> {
        if *self.currently_allocated_functions.read().await >= self.max_functions {
            return Err(RuntimeError::MaxFunctionDeplaymentReached);
        }

        let component = self
            .modules
            .get(&id)
            .ok_or(RuntimeError::ComponentNotFound)
            .map_err(|e| e)?
            .value()
            .component
            .clone();

        // Temporary
        let func_mem_size = self.memory / self.max_functions;
        let mut caf = self.currently_allocated_functions.write().await;
        *caf += 1;

        let lambda = Lambda::new(component.clone(), func_mem_size, tap_ip)
            .await
            .map_err(|e| RuntimeError::FunctionInitError(e.to_string()))?;

        let func_id = nanoid!(10, &nanoid::alphabet::SAFE);
        if self.functions.contains_key(&func_id) {
            return Err(RuntimeError::FunctionAlreadyInitialized);
        }

        self.functions.insert(
            func_id.clone(),
            Arc::new(RwLock::new(FunctionHandler {
                lambda,
                status: FunctionHandlerStatus::Ready,
            })),
        );

        Ok(func_id)
    }

    // Remove a lambda function from the registry
    pub async fn remove_function(&self, func_id: FunctionID) -> bool {
        if self.functions.contains_key(&func_id) {
            self.functions.remove(&func_id);
            return true;
        }
        false
    }

    // Exec function
    pub async fn exec_function(
        &self,
        func_id: FunctionID,
        args: &str,
    ) -> Result<String, RuntimeError> {
        // NOTE: For future use, check multiple function execute tracking
        let func_handler = self
            .functions
            .get(&func_id)
            .ok_or(RuntimeError::FunctionNotRegistered)
            .map_err(|e| e)?
            .value()
            .clone();

        // Exec function
        let lambda = &func_handler.read().await.lambda;
        let result = lambda
            .run(args)
            .await
            .map_err(|e| RuntimeError::FunctionExecError(e.to_string()))?;

        Ok(result)
    }

    pub async fn stop_function(&self, func_id: FunctionID) -> Result<(), RuntimeError> {
        // NOTE: For future use, check multiple function and execution tracking
        let func_handler = self
            .functions
            .get(&func_id)
            .ok_or(RuntimeError::FunctionNotRegistered)
            .map_err(|e| e)?
            .value()
            .clone();

        let lambda = &func_handler.read().await.lambda;
        lambda
            .stop()
            .await
            .map_err(|e| RuntimeError::FunctionStopError(e.to_string()))?;

        Ok(())
    }

    async fn get_engine(&self) -> Arc<Engine> {
        #[allow(unused_assignments)]
        let mut engine_rotatory_index = self.engine_rotatory_index.write().await;
        let index = if *engine_rotatory_index + 1 > self.vcpus {
            *engine_rotatory_index = 0;
            0
        } else {
            *engine_rotatory_index += 1;
            *engine_rotatory_index
        };
        Arc::clone(self.engines.get(index).unwrap())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        let runtime = Runtime::new()
            .set_cpus(4)
            .set_total_memory_size(1024 * 1024 * 100)
            .set_max_functions_number(25)
            .build()
            .unwrap();
        runtime
    }
}

#[cfg(test)]
mod test {
    use crate::runtime::lambda::Lambda;
    use crate::runtime::lambda_error::LambdaError;
    use crate::runtime::Runtime;
    use std::net::Ipv4Addr;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tokio;
    use wasmtime::component::Component;
    use wasmtime::*;

    fn get_crate_path() -> PathBuf {
        let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let wasm_path = Path::new(&crate_dir).join(Path::new(
            "resources/wasm_wasi_module_test_files/wasm_compiled",
        ));
        wasm_path
    }

    fn gen_engine(e_async: bool, e_epoch: bool, cranelift_opt_leve: OptLevel) -> Engine {
        let mut config = Config::new();
        config
            .async_support(e_async)
            .epoch_interruption(e_epoch)
            .cranelift_opt_level(cranelift_opt_leve);
        Engine::new(&config).unwrap()
    }

    fn load_component(engine: &Engine, path: PathBuf) -> wasmtime::component::Component {
        let component = Component::from_file(&engine, path).expect("Wasm module not found");
        component
    }

    async fn get_lambda(component_name: &str, mem_size: usize, tap_ip: Ipv4Addr) -> Lambda {
        let engine = Arc::new(gen_engine(true, true, OptLevel::Speed));
        let file = get_crate_path().join(component_name);
        let component = Arc::new(load_component(&engine, file));
        Lambda::new(component.clone(), mem_size, tap_ip)
            .await
            .unwrap()
    }

    // NOTE: Runtime test functions
    #[tokio::test]
    async fn create_runtime() {
        let _runtime = Runtime::new()
            .set_cpus(4)
            .set_total_memory_size(1024 * 1024 * 500)
            .build()
            .unwrap();
        assert!(true);
    }

    #[tokio::test]
    async fn register_and_remove_modules() {
        let runtime = Runtime::default();
        let modules_path = get_crate_path();
        let file = modules_path.join("exec_rust_lambda_function.wasm");
        let bytes = std::fs::read(file).unwrap();
        let module_id = runtime.register_module(bytes).await.unwrap();

        assert_eq!((), runtime.remove_module(module_id).await.unwrap());
    }

    #[tokio::test]
    async fn runtime_run_functions() {
        let runtime = Runtime::default();

        // Register module
        let modules_path = get_crate_path();
        let file = modules_path.join("exec_rust_lambda_function.wasm");
        let bytes = std::fs::read(file).unwrap();
        let module_id = runtime.register_module(bytes).await.unwrap();

        // Init function
        let func_id = runtime
            .init_function(module_id, Ipv4Addr::new(127, 0, 0, 1))
            .await
            .unwrap();

        // Run function
        let result = runtime.exec_function(func_id, "").await.unwrap();
        assert_eq!(result, "### TEST ###");
    }

    fn load_file(name: &str) -> Vec<u8> {
        let wasm_path = get_crate_path();
        let file_path = wasm_path.join(name);
        let file_bytes = std::fs::read(file_path).unwrap();
        file_bytes
    }

    #[tokio::test]
    async fn runtime_mutifunction_parallel_exec() {
        let runtime = Arc::new(Runtime::default());

        // Load files
        let bytes_exec_rust_lambda_function = load_file("exec_rust_lambda_function.wasm");
        let bytes_multi_function = load_file("multiple_function_exec.wasm");

        // Register modules
        let join_erlf = tokio::spawn({
            let runtime_rf = runtime.clone();
            async move {
                runtime_rf
                    .register_module(bytes_exec_rust_lambda_function)
                    .await
            }
        });
        let join_mf = tokio::spawn({
            let runtime_rf = runtime.clone();
            async move { runtime_rf.register_module(bytes_multi_function).await }
        });

        let (res_erlf, res_mf) = tokio::spawn(async move { (join_erlf.await, join_mf.await) })
            .await
            .unwrap();

        let module_id_erfl = res_erlf.unwrap().unwrap();
        let module_id_mf = res_mf.unwrap().unwrap();

        // Init functions
    }

    // #[tokio::test]
    // async fn runtime_multiple_function_exec() {
    //     let mut runtime = Runtime::default();
    //     let engine = gen_engine(true, true, OptLevel::SpeedAndSize);
    //
    //     // Load files
    //     let modules_path = get_crate_path();
    //     let file_exec_rust_lambda_function =
    //         std::fs::read(modules_path.clone().join("exec_rust_lambda_function.wasm")).unwrap();
    //     let file_multi_function = std::fs::read(
    //         modules_path
    //             .clone()
    //             .join("multi_function_exec_one_interrupt.wasm"),
    //     )
    //     .unwrap();
    //
    //     // Register modules
    //     let module_exec_rust_id = runtime
    //         .register_module(file_exec_rust_lambda_function)
    //         .await
    //         .unwrap();
    //     let module_multi_func_id = runtime.register_module(file_multi_function).await.unwrap();
    //
    //     // Init function
    //     let func_exec_rust_id = runtime
    //         .init_function(module_exec_rust_id, Ipv4Addr::new(127, 0, 0, 1))
    //         .await
    //         .unwrap();
    //
    //     let func_multi_func_id = runtime
    //         .init_function(module_multi_func_id, Ipv4Addr::new(127, 0, 0, 1))
    //         .await
    //         .unwrap();
    //
    //     // Call che functions
    //     let runtime_ref = Arc::new(RwLock::new(runtime));
    //     let runtime_1 = runtime_ref.clone();
    //     let runtime_2 = runtime_ref.clone();
    //     let runtime_3 = runtime_ref.clone();
    //
    //     let join_handler_1 = tokio::spawn(async move {
    //         runtime_1
    //             .read()
    //             .unwrap()
    //             .exec_function(&func_exec_rust_id, "")
    //             .await
    //     })
    //     .await
    //     .unwrap();
    //
    //     let func_multi_func_id_1 = func_multi_func_id.clone();
    //     let join_handler_2 = tokio::spawn(async move {
    //         runtime_2
    //             .read()
    //             .unwrap()
    //             .exec_function(&func_multi_func_id_1, "f,e,d,c,b,a")
    //             .await
    //     })
    //     .await
    //     .unwrap();
    //
    //     let func_multi_func_id_2 = func_multi_func_id.clone();
    //     let join_handler_3 = tokio::spawn(async move {
    //         runtime_3
    //             .read()
    //             .unwrap()
    //             .exec_function(&func_multi_func_id_2, "e,d,c,b,a")
    //             .await
    //     })
    //     .await
    //     .unwrap();
    // }

    // NOTE: Lambda test functions
    #[tokio::test]
    async fn exec_rust_lambda_function() {
        let lambda = get_lambda(
            "exec_rust_lambda_function.wasm",
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await;

        let res = lambda.run("").await.unwrap();
        assert_eq!(res, "### TEST ###");
    }

    #[tokio::test]
    async fn stop_infinite_loop_function() {
        let lambda = Arc::new(tokio::sync::RwLock::new(
            get_lambda(
                "stop_infinite_loop.wasm",
                1024 * 1204 * 2,
                Ipv4Addr::new(127, 0, 0, 1),
            )
            .await,
        ));

        let handler = tokio::spawn({
            let lambda_ref = lambda.clone();
            async move {
                let lref = lambda_ref.read().await;
                lref.run("").await
            }
        });
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let _ = lambda.read().await.stop().await;
        assert_eq!(Err(LambdaError::ForceStop), handler.await.unwrap());
    }

    #[tokio::test]
    async fn multiple_function_exec() {
        let lambda = Arc::new(tokio::sync::RwLock::new(
            get_lambda(
                "multiple_function_exec.wasm",
                1024 * 1024 * 5,
                Ipv4Addr::new(127, 0, 0, 1),
            )
            .await,
        ));

        let handler_one = tokio::spawn({
            let lambda_rwl = lambda.clone();
            async move {
                let lambda = lambda_rwl.read().await;
                lambda.run("f,e,d,c,b,a").await
            }
        });

        let handler_two = tokio::spawn({
            let lambda_rwl = lambda.clone();
            async move {
                let lambda = lambda_rwl.read().await;
                lambda.run("e,d,c,b,a").await
            }
        });

        let (result_one, result_two) =
            tokio::spawn(async move { (handler_one.await, handler_two.await) })
                .await
                .unwrap();
        let result_one = result_one.unwrap().unwrap();
        let result_two = result_two.unwrap().unwrap();
        assert_eq!(result_one, "[a,b,c,d,e,f]");
        assert_eq!(result_two, "[a,b,c,d,e]");
    }

    #[tokio::test]
    async fn tcp_udp_bind_to_not_allowed_ip() {
        let lambda = get_lambda(
            "tcp_udp_bind_to_not_allowed_ip.wasm",
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await;

        // Allowed ip for tcp/udp
        assert_eq!(
            "### TCP ###",
            lambda.run("TCP,127.0.0.1:50400").await.unwrap()
        );
        assert_eq!(
            "### UDP ###",
            lambda.run("UDP,127.0.0.1:50400").await.unwrap()
        );

        // Not allowed ip for tcp/udp
        assert_eq!(
            Err(LambdaError::FunctionExecError),
            lambda.run("TCP,192.168.2.2.3:50300").await
        );
        assert_eq!(
            Err(LambdaError::FunctionExecError),
            lambda.run("UDP,192.168.2.2.3:50300").await
        );
    }
}
