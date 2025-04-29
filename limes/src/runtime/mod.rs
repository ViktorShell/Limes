use crc32fast::Hasher;
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

// pub struct RuntimeBuilder {
//     vcpus: Option<usize>,
//     memory: Option<usize>,
//     max_functions: Option<usize>,
//     currently_allocated_functions: Option<usize>,
// }
//
// impl RuntimeBuilder {
//     pub fn set_cpus(&mut self, vcpus: usize) -> &mut Self {
//         self.vcpus = Some(vcpus);
//         self
//     }
//
//     pub fn set_total_memory_size(&mut self, memory_size: usize) -> &mut Self {
//         self.memory = Some(memory_size);
//         self
//     }
//
//     pub fn set_max_functions_number(&mut self, size: usize) -> &mut Self {
//         self.max_functions = Some(size);
//         self
//     }
//
//     pub fn build(&self) -> Result<Runtime, RuntimeError> {
//         let mut engines_config = Config::new();
//         engines_config
//             .async_support(true)
//             .wasm_component_model(true)
//             .cranelift_opt_level(OptLevel::SpeedAndSize);
//         let engines: Vec<Arc<Engine>> = self.gen_engines(self.vcpus.unwrap(), &engines_config)?;
//
//         Ok(Runtime {
//             vcpus: self.vcpus.unwrap(),
//             memory: self.memory.unwrap(),
//             max_functions: self.max_functions.unwrap(),
//             currently_allocated_functions: self.currently_allocated_functions.unwrap(),
//             engines,
//             engine_rotatory_index: 0,
//             modules: HashMap::new(),
//             functions: HashMap::new(),
//         })
//     }
//
//     fn gen_engines(
//         &self,
//         num: usize,
//         config: &wasmtime::Config,
//     ) -> Result<Vec<Arc<Engine>>, RuntimeError> {
//         let mut engines = Vec::new();
//         for _ in 0..num {
//             let engine = Arc::new(Engine::new(config).map_err(|_| RuntimeError::EngineInitError)?);
//             engines.push(engine);
//         }
//         Ok(engines)
//     }
// }
//
// pub struct FunctionHandler {
//     lambda: Lambda, // FIX: QUI DEVI METTERE IL RWLOCK, E'
//     status: FunctionHandlerStatus,
// }
//
// #[derive(PartialEq)]
// pub enum FunctionHandlerStatus {
//     Ready,
//     Running,
//     Stopped,
// }
//
// pub struct Runtime {
//     vcpus: usize,
//     memory: usize,
//     max_functions: usize,
//     currently_allocated_functions: usize,
//     engines: Vec<Arc<Engine>>,
//     // Temporary solution for engine deployment, need a HeapMin for a good queue
//     engine_rotatory_index: usize,
//     modules: HashMap<String, Arc<Component>>,
//     functions: HashMap<String, Arc<RwLock<FunctionHandler>>>,
// }
//
// impl Runtime {
//     // Costruct the Runtime
//     pub fn new() -> RuntimeBuilder {
//         RuntimeBuilder {
//             vcpus: Some(1),
//             memory: Some(1024 * 1024 * 10), // Instance for 10 instance of 2Mb each
//             max_functions: Some(1000),
//             currently_allocated_functions: Some(0),
//         }
//     }
//
//     // Get the code in byteform, try to compile it and save it a sqa db and make it ready for
//     // deployment, return an uuid of the module
//     // NOTE: Add the following
//     // - Register to DB
//     // - Logger
//     pub async fn register_module(&mut self, module: Vec<u8>) -> Result<String, RuntimeError> {
//         let engine = self.get_engine();
//         let mut hasher = Hasher::new();
//         hasher.update(&module);
//         let module_id = hasher.finalize().to_string();
//         if self.modules.contains_key(&module_id) {
//             return Err(RuntimeError::ModuleAlreadyReg);
//         }
//
//         let component = Arc::new(
//             wasmtime::component::Component::from_binary(&engine, &module)
//                 .map_err(|_| RuntimeError::ComponentBuildError)?,
//         );
//         self.modules.insert(module_id.clone(), component);
//
//         Ok(module_id)
//     }
//
//     // Remove a registered module
//     // NOTE: Add the following
//     // - Register to DB
//     // - Logger
//     pub async fn remove_module(&mut self, module_id: String) -> bool {
//         if self.modules.contains_key(&module_id) {
//             self.modules.remove(&module_id);
//             return true;
//         }
//         false
//     }
//
//     // Initialize the function
//     // Doesn't need to check if already exists, because you can call only one a time
//     // Generate one
//     // return a function_id
//     pub async fn init_function(
//         &mut self,
//         module_id: String,
//         tap_ip: Ipv4Addr,
//     ) -> Result<String, RuntimeError> {
//         let component = self.get_component(module_id)?;
//
//         if self.currently_allocated_functions >= self.max_functions {
//             return Err(RuntimeError::MaxFunctionDeplaymentReached);
//         }
//         let function_memory = self.memory / self.max_functions;
//         self.currently_allocated_functions += 1;
//         let lambda = Lambda::new(component, function_memory, tap_ip)
//             .await
//             .map_err(|e| RuntimeError::FunctionInitError(e.to_string()))?;
//
//         let function_handler = FunctionHandler {
//             lambda,
//             status: FunctionHandlerStatus::Ready,
//         };
//
//         let func_id = nanoid!(20, &nanoid::alphabet::SAFE);
//         if let Some(_) = self
//             .functions
//             .insert(func_id.clone(), Arc::new(RwLock::new(function_handler)))
//         {
//             return Err(RuntimeError::FunctionAlreadyInitialized);
//         }
//
//         Ok(func_id)
//     }
//
//     // Remove a lambda function from the registry
//     // Search for the function
//     // if present remove it
//     // else return error
//     pub async fn remove_function(&mut self, func_id: &str) -> bool {
//         if self.functions.contains_key(func_id) {
//             self.functions.remove(func_id);
//             return true;
//         }
//         return false;
//     }
//
//     // Init the function and execute it
//     // Execute lambda with args
//     pub async fn exec_function(&self, func_id: &str, args: &str) -> Result<String, RuntimeError> {
//         // Check if initialized
//         let func_handler = match self.functions.get(func_id) {
//             Some(func_handler) => func_handler,
//             None => {
//                 return Err(RuntimeError::FunctionExecError(
//                     "Function not initialized".to_string(),
//                 ))
//             }
//         };
//
//         // Check status
//         match func_handler.read() {
//             Ok(handler) => {
//                 if (*handler).status != FunctionHandlerStatus::Ready {
//                     return Err(RuntimeError::FunctionExecError(
//                         "Function is not in a ready state".to_string(),
//                     ));
//                 }
//             }
//             Err(e) => return Err(RuntimeError::FunctionExecError(e.to_string())),
//         };
//
//         // Change status & exec func
//         let result = match func_handler.write() {
//             Ok(mut handler) => {
//                 (*handler).status = FunctionHandlerStatus::Running;
//                 let result = (*handler)
//                     .lambda
//                     .run(args)
//                     .await
//                     .map_err(|e| RuntimeError::FunctionExecError(e.to_string()))?;
//                 result
//             }
//             Err(e) => return Err(RuntimeError::FunctionExecError(e.to_string())),
//         };
//
//         // Return result
//         Ok(result)
//     }
//
//     // Interrupt the execution of a lambda
//     pub async fn stop_function(&self, func_id: &str) -> Result<(), RuntimeError> {
//         // Check if initialized
//         let func_handler = match self.functions.get(func_id) {
//             Some(func_handler) => func_handler,
//             None => {
//                 return Err(RuntimeError::FunctionExecError(
//                     "Function not initialized".to_string(),
//                 ))
//             }
//         };
//
//         let func_handler = match func_handler.read() {
//             Ok(handler) => handler,
//             Err(e) => return Err(RuntimeError::FunctionStopError(e.to_string())),
//         };
//
//         match func_handler.lambda.stop().await {
//             Err(e) => return Err(RuntimeError::FunctionStopError(e.to_string())),
//             _ => Ok(()),
//         }
//     }
//
//     fn get_engine(&mut self) -> Arc<Engine> {
//         #[allow(unused_assignments)]
//         let mut index: usize = 0;
//         if self.engine_rotatory_index + 1 > self.vcpus {
//             self.engine_rotatory_index = 0;
//             index = 0;
//         } else {
//             self.engine_rotatory_index = self.engine_rotatory_index + 1;
//             index = self.engine_rotatory_index;
//         }
//         Arc::clone(self.engines.get(index).unwrap())
//     }
//
//     fn get_component(&mut self, module_id: String) -> Result<Arc<Component>, RuntimeError> {
//         let component = self.modules.get(&module_id);
//         if let Some(component) = component {
//             return Ok(Arc::clone(component));
//         }
//         Err(RuntimeError::ComponentNotFound)
//     }
// }
//
// impl Default for Runtime {
//     fn default() -> Self {
//         let runtime = Runtime::new()
//             .set_cpus(4)
//             .set_total_memory_size(1024 * 1024 * 100)
//             .set_max_functions_number(25)
//             .build()
//             .unwrap();
//         runtime
//     }
// }

#[cfg(test)]
mod test {
    use crate::runtime::lambda::Lambda;
    use crate::runtime::lambda_error::LambdaError;
    // use crate::runtime::Runtime;
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
        Lambda::new(engine.clone(), component.clone(), mem_size, tap_ip)
            .await
            .unwrap()
    }

    // // NOTE: Runtime test functions
    // #[tokio::test]
    // async fn create_runtime() {
    //     let runtime = Runtime::new()
    //         .set_cpus(4)
    //         .set_total_memory_size(1024 * 1024 * 500)
    //         .build();
    //     if let Ok(_runtime) = runtime {
    //         assert!(true);
    //         return;
    //     }
    //     assert!(false)
    // }
    //
    // #[tokio::test]
    // async fn register_and_remove_modules() {
    //     let mut runtime = Runtime::new()
    //         .set_cpus(4)
    //         .set_total_memory_size(1024 * 1024 * 100)
    //         .build()
    //         .unwrap();
    //
    //     let modules_path = get_crate_path();
    //     let file = modules_path.join("exec_rust_lambda_function.wasm");
    //     let bytes = std::fs::read(file).unwrap();
    //     let module_id = runtime.register_module(bytes).await.unwrap();
    //     if runtime.remove_module(module_id).await {
    //         assert!(true);
    //         return;
    //     } else {
    //         assert!(false);
    //         return;
    //     }
    // }
    //
    // #[tokio::test]
    // async fn runtime_run_functions() {
    //     let mut runtime = Runtime::default();
    //
    //     // Register module
    //     let modules_path = get_crate_path();
    //     let file = modules_path.join("exec_rust_lambda_function.wasm");
    //     let bytes = std::fs::read(file).unwrap();
    //     let module_id = runtime.register_module(bytes).await.unwrap();
    //
    //     // Init function
    //     let func_id = runtime
    //         .init_function(module_id, Ipv4Addr::new(127, 0, 0, 1))
    //         .await
    //         .unwrap();
    //
    //     // Run function
    //     let result = runtime.exec_function(&func_id, "").await.unwrap();
    //     assert_eq!(result, "### TEST ###");
    // }
    //
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
