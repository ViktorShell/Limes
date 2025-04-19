use lambda::Lambda;
use lambda_error::LambdaError;
use nanoid::alphabet;
use nanoid::nanoid;
use phf::map::Map;
use std::sync::Arc;
use uuid::uuid;
use wasmtime::component::Component;
use wasmtime::EngineWeak;

pub mod lambda;
pub mod lambda_error;
pub mod runtime_error;

pub struct RuntimeBuilder {
    vcpus: Option<usize>,
    memory: Option<usize>,
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

    pub fn build(&self) -> Runtime {
        Runtime {
            vcpus: self.vcpus.unwrap(),
            memory: self.memory.unwrap(),
            engines: Vec::new(),
            modules: Map::new(),
            lambda_ready_exec: Map::new(),
            lambda_stop_exec: Map::new(),
            lambda_running_function: Map::new(),
        }
    }
}

#[allow(dead_code)]
pub struct Runtime {
    vcpus: usize,
    memory: usize,
    engines: Vec<EngineWeak>,
    modules: Map<String, Arc<Component>>,
    lambda_ready_exec: Map<String, Lambda>,
    lambda_stop_exec: Map<String, Box<dyn Fn() -> Result<(), LambdaError>>>,
    lambda_running_function: Map<String, bool>,
}

#[allow(dead_code)]
impl Runtime {
    // Costruct the Runtime
    pub fn new() -> RuntimeBuilder {
        RuntimeBuilder {
            vcpus: Some(1),
            memory: Some(1024 * 1024 * 10), // Instance for 10 instance of 2Mb each
        }
    }

    // Get the code in byteform, try to compile it and save it a sqa db and make it ready for
    // deployment, return an uuid of the module
    pub async fn register_module() {
        todo!();
    }

    // Remove a registered module
    pub async fn remove_module() {
        todo!();
    }

    // Initialize a lambda function and return an nanoid
    pub async fn init_lambda() {
        todo!();
    }

    // Remove a lambda function from the registry
    pub async fn remove_lambda() {
        todo!();
    }

    // Execute lambda with args
    pub async fn run_lambda() {
        todo!();
    }

    // Interrupt the execution of a lambda
    pub async fn stop_lambda() {
        todo!();
    }

    // check if a lambda function is currently running
    fn is_lambda_running() {
        todo!()
    }
}

impl Default for Runtime {
    fn default() -> Self {
        todo!();
    }
}

#[cfg(test)]
mod test {
    use crate::runtime::lambda::Lambda;
    use crate::runtime::lambda_error::LambdaError;
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
        let (mut lambda_right_tcp, _) = get_lambda(
            "tcp_udp_bind_to_not_allowed_ip.wasm",
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await;
        let (mut lambda_wrong_tcp, _) = get_lambda(
            "tcp_udp_bind_to_not_allowed_ip.wasm",
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await;
        let (mut lambda_right_udp, _) = get_lambda(
            "tcp_udp_bind_to_not_allowed_ip.wasm",
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await;
        let (mut lambda_wrong_udp, _) = get_lambda(
            "tcp_udp_bind_to_not_allowed_ip.wasm",
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await;

        // allowed ip for tcp/udp
        assert_eq!(
            "### TCP ###",
            lambda_right_tcp.run("TCP,127.0.0.1:50402").await.unwrap()
        );
        assert_eq!(
            "### UDP ###",
            lambda_right_udp.run("UDP,127.0.0.1:50403").await.unwrap()
        );

        // not allowed ip for tcp/udp
        assert_eq!(
            LambdaError::FunctionExecError,
            lambda_wrong_tcp
                .run("TCP,192.168.1.2:50400")
                .await
                .unwrap_err()
        );
        assert_eq!(
            LambdaError::FunctionExecError,
            lambda_wrong_udp
                .run("UDP,192.168.1.2:50401")
                .await
                .unwrap_err()
        );
    }
}
