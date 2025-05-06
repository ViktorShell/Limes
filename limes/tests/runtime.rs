use limes::runtime::Runtime;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio;

fn get_crate_path() -> PathBuf {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let wasm_path = Path::new(&crate_dir).join(Path::new(
        "resources/wasm_wasi_module_test_files/wasm_compiled",
    ));
    wasm_path
}

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
    let tap_ip = Ipv4Addr::new(127, 0, 0, 1);
    let func_id_erfl = runtime.init_function(module_id_erfl, tap_ip).await.unwrap();
    let func_id_mf = Arc::new(runtime.init_function(module_id_mf, tap_ip).await.unwrap());

    // Exec functions
    let join_res_erfl = tokio::spawn({
        let runtime_r = runtime.clone();
        async move { runtime_r.exec_function(func_id_erfl, "").await }
    });
    let join_res_mf_1 = tokio::spawn({
        let runtime_r = runtime.clone();
        let func_id_r = func_id_mf.clone();
        async move {
            runtime_r
                .exec_function(func_id_r.to_string(), "f,e,c,d,a")
                .await
        }
    });
    let join_res_mf_2 = tokio::spawn({
        let runtime_r = runtime.clone();
        let func_id_r = func_id_mf.clone();
        async move {
            runtime_r
                .exec_function(func_id_r.to_string(), "f,e,c,d,a,x")
                .await
        }
    });

    let (r_erfl, r_mf_1, r_mf_2) = tokio::spawn(async move {
        (
            join_res_erfl.await,
            join_res_mf_1.await,
            join_res_mf_2.await,
        )
    })
    .await
    .unwrap();
    let r_erfl = r_erfl.unwrap().unwrap();
    let r_mf_1 = r_mf_1.unwrap().unwrap();
    let r_mf_2 = r_mf_2.unwrap().unwrap();

    assert_eq!(r_erfl, "### TEST ###");
    assert_eq!(r_mf_1, "[a,c,d,e,f]");
    assert_eq!(r_mf_2, "[a,c,d,e,f,x]");
}
