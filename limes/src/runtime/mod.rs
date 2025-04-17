use phf::map::Map;
use uuid::Uuid;
pub mod lambda;
pub mod lambda_error;

#[allow(dead_code)]
struct Runtime {
    vcpus: u8,
    memory: u32,
    modules: Map<Uuid, String>,
    ready_modules: Map<Uuid, String>,
    running_modules: Map<Uuid, String>,
}
#[allow(dead_code)]
impl Runtime {
    pub fn new() {
        todo!();
    }

    pub fn register() {
        todo!();
    }

    pub fn remove() {
        todo!();
    }

    pub async fn start() {
        todo!();
    }

    pub async fn stop() {
        todo!();
    }

    pub async fn is_running() {
        todo!();
    }

    pub async fn is_ready() {
        todo!();
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

    #[tokio::test]
    async fn exec_rust_lambda_function() {
        let engine = Arc::new(gen_engine(true, true, OptLevel::Speed));
        let file = get_crate_path().join("exec_rust_lambda_function.wasm");
        let component = Arc::new(load_component(&engine, file));
        let (mut lambda, _) = Lambda::new(
            engine,
            component,
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await
        .unwrap();

        let res = lambda.run("").await.unwrap();
        assert_eq!(res, "### TEST ###");
    }

    #[tokio::test]
    async fn stop_infinite_loop_function() {
        let engine = Arc::new(gen_engine(true, true, OptLevel::Speed));
        let file = get_crate_path().join("stop_infinite_loop.wasm");
        let component = Arc::new(load_component(&engine, file));
        let (mut lambda, stop_func) = Lambda::new(
            engine,
            component,
            1024 * 1204 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await
        .unwrap();

        let handler = tokio::spawn(async move { lambda.run("").await });
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        if !handler.is_finished() {
            stop_func().unwrap();
            if let Err(LambdaError::ForceStop) = handler.await.unwrap() {
                assert!(true);
                return; // assert just exec a check it doesn't stop the function
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
        let engine = Arc::new(gen_engine(true, true, OptLevel::Speed));
        let file = get_crate_path().join("tcp_udp_bind_to_not_allowed_ip.wasm");
        let component = Arc::new(load_component(&engine, file));
        let (mut lambda, _) = Lambda::new(
            engine,
            component,
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await
        .unwrap();

        // FIX: Implement Rebuild of WasiCtx on every run
        // Try allowed
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
            lambda.run("TCP,192.168.112.2:50400").await.unwrap_err()
        );
        assert_eq!(
            LambdaError::FunctionExecError,
            lambda.run("UDP,192.168.112.2:50401").await.unwrap_err()
        );
    }
}
