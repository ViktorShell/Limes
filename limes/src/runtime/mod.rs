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
    use wasmtime::component::{Component, Linker, ResourceTable};
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

    // #[tokio::test]
    // async fn stop_infinite_loop_function() {
    //     todo!()
    // }
    //
    // #[tokio::test]
    // async fn multiple_function_exec_one_interrupt() {
    //     todo!()
    // }
    //
    // #[tokio::test]
    // async fn tcp_udp_bind_to_not_allowed_ip() {
    //     todo!()
    // }
    //
    // #[tokio::test]
    // async fn stdio_output() {
    //     todo!()
    // }
}
