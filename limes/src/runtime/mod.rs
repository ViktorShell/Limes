use crc32fast::Hasher;
use lambda::Lambda;
use lambda_error::LambdaError;
use nanoid::alphabet;
use nanoid::nanoid;
use runtime_error::RuntimeError;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::RwLock;
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
            currently_allocated_functions: self.currently_allocated_functions.unwrap(),
            engines,
            engine_rotatory_index: 0,
            modules: HashMap::new(),
            functions: HashMap::new(),
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

pub struct FunctionHandler {
    lambda: Lambda,
    stop_func: Arc<Box<dyn Fn() -> Result<(), LambdaError>>>,
    status: FunctionHandlerStatus,
}

#[derive(PartialEq)]
pub enum FunctionHandlerStatus {
    Ready,
    Running,
    Stopped,
}

pub struct Runtime {
    vcpus: usize,
    memory: usize,
    max_functions: usize,
    currently_allocated_functions: usize,
    engines: Vec<Arc<Engine>>,
    // Temporary solution for engine deployment, need a HeapMin for a good queue
    engine_rotatory_index: usize,
    modules: HashMap<String, Arc<Component>>,
    functions: HashMap<String, Arc<RwLock<FunctionHandler>>>,
}

impl Runtime {
    // Costruct the Runtime
    pub fn new() -> RuntimeBuilder {
        RuntimeBuilder {
            vcpus: Some(1),
            memory: Some(1024 * 1024 * 10), // Instance for 10 instance of 2Mb each
            max_functions: Some(1000),
            currently_allocated_functions: Some(0),
        }
    }

    // Get the code in byteform, try to compile it and save it a sqa db and make it ready for
    // deployment, return an uuid of the module
    // NOTE: Add the following
    // - Register to DB
    // - Logger
    pub async fn register_module(&mut self, module: Vec<u8>) -> Result<String, RuntimeError> {
        let engine = self.get_engine();
        let mut hasher = Hasher::new();
        hasher.update(&module);
        let module_id = hasher.finalize().to_string();
        if self.modules.contains_key(&module_id) {
            return Err(RuntimeError::ModuleAlreadyReg);
        }

        let component = Arc::new(
            wasmtime::component::Component::from_binary(&engine, &module)
                .map_err(|_| RuntimeError::ComponentBuildError)?,
        );
        self.modules.insert(module_id.clone(), component);

        Ok(module_id)
    }

    // Remove a registered module
    // NOTE: Add the following
    // - Register to DB
    // - Logger
    pub async fn remove_module(&mut self, module_id: String) -> bool {
        if self.modules.contains_key(&module_id) {
            self.modules.remove(&module_id);
            return true;
        }
        false
    }

    // Initialize the function
    // Doesn't need to check if already exists, because you can call only one a time
    // Generate one
    // return a function_id
    pub async fn init_function(
        &mut self,
        module_id: String,
        tap_ip: Ipv4Addr,
    ) -> Result<String, RuntimeError> {
        let component = self.get_component(module_id)?;
        let engine = Arc::new(*(component.engine()));

        if self.currently_allocated_functions >= self.max_functions {
            return Err(RuntimeError::MaxFunctionDeplaymentReached);
        }
        let function_memory = self.memory / self.max_functions;
        self.currently_allocated_functions += 1;
        let (lambda, stop_func) = Lambda::new(engine, component, function_memory, tap_ip)
            .await
            .map_err(|e| RuntimeError::FunctionInitError(e.to_string()))?;

        let function_handler = FunctionHandler {
            lambda,
            stop_func: Arc::new(Box::new(stop_func)),
            status: FunctionHandlerStatus::Ready,
        };

        let func_id = nanoid!(20, &nanoid::alphabet::SAFE);
        if let Some(_) = self
            .functions
            .insert(func_id.clone(), Arc::new(RwLock::new(function_handler)))
        {
            return Err(RuntimeError::FunctionAlreadyInitialized);
        }

        Ok(func_id)
    }

    // Remove a lambda function from the registry
    // Search for the function
    // if present remove it
    // else return error
    pub async fn remove_function(&mut self, func_id: &str) -> bool {
        if self.functions.contains_key(func_id) {
            self.functions.remove(func_id);
            return true;
        }
        return false;
    }

    // Init the function and execute it
    // Execute lambda with args
    pub async fn exec_function(
        &mut self,
        func_id: &str,
        args: &str,
    ) -> Result<String, RuntimeError> {
        // Check if initialized
        let func_handler = match self.functions.get(func_id) {
            Some(func_handler) => func_handler,
            None => {
                return Err(RuntimeError::FunctionExecError(
                    "Function not initialized".to_string(),
                ))
            }
        };

        // Check status
        match func_handler.read() {
            Ok(handler) => {
                if (*handler).status != FunctionHandlerStatus::Ready {
                    return Err(RuntimeError::FunctionExecError(
                        "Function is not in a ready state".to_string(),
                    ));
                }
            }
            Err(e) => return Err(RuntimeError::FunctionExecError(e.to_string())),
        };

        // Change status & exec func
        let result = match func_handler.write() {
            Ok(mut handler) => {
                (*handler).status = FunctionHandlerStatus::Running;
                let result = (*handler)
                    .lambda
                    .run(args)
                    .await
                    .map_err(|e| RuntimeError::FunctionExecError(e.to_string()))?;
                result
            }
            Err(e) => return Err(RuntimeError::FunctionExecError(e.to_string())),
        };

        // Return result
        Ok(result)
    }

    // Interrupt the execution of a lambda
    pub async fn stop_function(&self, func_id: &str) -> Result<(), RuntimeError> {
        // Check if initialized
        let func_handler = match self.functions.get(func_id) {
            Some(func_handler) => func_handler,
            None => {
                return Err(RuntimeError::FunctionExecError(
                    "Function not initialized".to_string(),
                ))
            }
        };

        let stop_func = match func_handler.read() {
            Ok(handler) => (*handler).stop_func.clone(),
            Err(e) => return Err(RuntimeError::FunctionStopError(e.to_string())),
        };

        match stop_func() {
            Err(e) => return Err(RuntimeError::FunctionStopError(e.to_string())),
            _ => Ok(()),
        }
    }

    fn get_engine(&mut self) -> Arc<Engine> {
        #[allow(unused_assignments)]
        let mut index: usize = 0;
        if self.engine_rotatory_index + 1 > self.vcpus {
            self.engine_rotatory_index = 0;
            index = 0;
        } else {
            self.engine_rotatory_index = self.engine_rotatory_index + 1;
            index = self.engine_rotatory_index;
        }
        Arc::clone(self.engines.get(index).unwrap())
    }

    fn get_component(&mut self, module_id: String) -> Result<Arc<Component>, RuntimeError> {
        let component = self.modules.get(&module_id);
        if let Some(component) = component {
            return Ok(Arc::clone(component));
        }
        Err(RuntimeError::ComponentNotFound)
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

    async fn get_lambda(
        component_name: &str,
        mem_size: usize,
        tap_ip: Ipv4Addr,
    ) -> (Lambda, impl Fn() -> Result<(), LambdaError>) {
        let engine = Arc::new(gen_engine(true, true, OptLevel::Speed));
        let file = get_crate_path().join(component_name);
        let component = Arc::new(load_component(&engine, file));
        Lambda::new(engine, component, mem_size, tap_ip)
            .await
            .unwrap()
    }

    // NOTE: Runtime test functions
    #[tokio::test]
    async fn create_runtime() {
        let runtime = Runtime::new()
            .set_cpus(4)
            .set_total_memory_size(1024 * 1024 * 500)
            .build();
        if let Ok(_runtime) = runtime {
            assert!(true);
            return;
        }
        assert!(false)
    }

    #[tokio::test]
    async fn register_and_remove_modules() {
        let mut runtime = Runtime::new()
            .set_cpus(4)
            .set_total_memory_size(1024 * 1024 * 100)
            .build()
            .unwrap();

        let modules_path = get_crate_path();
        let file = modules_path.join("exec_rust_lambda_function.wasm");
        let bytes = std::fs::read(file).unwrap();
        let module_id = runtime.register_module(bytes).await.unwrap();
        if runtime.remove_module(module_id).await {
            assert!(true);
            return;
        } else {
            assert!(false);
            return;
        }
    }

    #[tokio::test]
    async fn runtime_run_functions() {
        let mut runtime = Runtime::default();

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
        let result = runtime.exec_function(&func_id, "").await.unwrap();
        assert_eq!(result, "### TEST ###");
    }

    // NOTE: Lambda test functions
    #[tokio::test]
    async fn exec_rust_lambda_function() {
        let (mut lambda, _) = get_lambda(
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
        let (mut lambda, stop_func) = get_lambda(
            "stop_infinite_loop.wasm",
            1024 * 1204 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await;

        let handler = tokio::spawn(async move { lambda.run("").await });
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        if !handler.is_finished() {
            stop_func().unwrap();
            if let Err(LambdaError::ForceStop) = handler.await.unwrap() {
                assert!(true);
                return; // assert just exec a check on the value and doesn't stop the function, this little error
                        // make me rethink about my career choose
            }
        }
        assert!(false);
    }

    #[tokio::test]
    async fn multiple_function_exec_one_interrupt() {
        let engine = Arc::new(gen_engine(true, true, OptLevel::Speed));
        let file = get_crate_path().join("multi_function_exec_one_interrupt.wasm");
        let component = Arc::new(load_component(&engine, file));

        let (mut lambda_run, _) = Lambda::new(
            engine.clone(),
            component.clone(),
            1024 * 1204 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await
        .unwrap();

        let (mut lambda_stop, stop_func) = Lambda::new(
            engine.clone(),
            component.clone(),
            1024 * 1204 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await
        .unwrap();

        // Exec both, stop one and get result from the other a,b,c,d,e,f
        let handler_run = tokio::spawn(async move { lambda_run.run("f,e,d,c,b,a").await });
        let handler_stop = tokio::spawn(async move { lambda_stop.run("f,e,d,c,b,a").await });
        assert_eq!(stop_func().unwrap(), ());
        assert_eq!(Err(LambdaError::ForceStop), handler_stop.await.unwrap());
        let result = handler_run.await.unwrap().unwrap();
        assert_eq!(result, "[a,b,c,d,e,f]");
    }

    #[tokio::test]
    async fn tcp_udp_bind_to_not_allowed_ip() {
        let (mut lambda, _) = get_lambda(
            "tcp_udp_bind_to_not_allowed_ip.wasm",
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await;

        // allowed ip for tcp/udp
        assert_eq!(
            "### TCP ###",
            lambda.run("TCP,127.0.0.1:50402").await.unwrap()
        );
        assert_eq!(
            "### UDP ###",
            lambda.run("UDP,127.0.0.1:50403").await.unwrap()
        );

        // not allowed ip for tcp/udp
        assert_eq!(
            LambdaError::FunctionExecError,
            lambda.run("TCP,192.168.1.2:50400").await.unwrap_err()
        );
        assert_eq!(
            LambdaError::FunctionExecError,
            lambda.run("UDP,192.168.1.2:50401").await.unwrap_err()
        );
    }
}
