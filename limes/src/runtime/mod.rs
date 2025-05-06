use crc32fast::Hasher;
use dashmap::DashMap;
use lambda::Lambda;
use nanoid::nanoid;
use runtime_error::RuntimeError;
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
            .set_cpus(1)
            .set_total_memory_size(1024 * 1024 * 100)
            .set_max_functions_number(25)
            .build()
            .unwrap();
        runtime
    }
}
