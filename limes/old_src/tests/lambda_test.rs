use limes::runtime::lambda::{self, Lambda};
use limes::runtime::lambda_error::LambdaError;
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
    let wasi_flags = lambda::WasiFlags::default();
    Lambda::new(component.clone(), mem_size, tap_ip, wasi_flags)
        .await
        .unwrap()
}

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
