use limes::runtime::lambda::WasiFlags;
use limes::tools::loader;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use wasmtime_wasi::DirPerms;
use wasmtime_wasi::FilePerms;

#[tokio::test]
async fn mandelbrotset() {
    let mut cargo_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cargo_dir_path.push("resources/benchmarks/mandelbrotset/");
    let mut wasm_path = cargo_dir_path.clone();
    wasm_path.push("mandelbrotset.wasm");
    let mut img_dir = cargo_dir_path.clone();
    img_dir.push("images");
    let mut file_map = HashMap::new();
    file_map.insert(
        img_dir.to_str().unwrap().to_string(),
        ("./".to_string(), DirPerms::all(), FilePerms::all()),
    );
    let wasm_flags = WasiFlags::new(Some(()), Some(file_map));
    let lambda = loader::build_lambda_from_file(
        &wasm_path,
        1024 * 1024 * 1000,
        Ipv4Addr::new(127, 0, 0, 1),
        wasm_flags,
    )
    .await
    .unwrap();
    lambda.run("").await.unwrap();
    assert!(true)
}

#[tokio::test]
async fn mandelbrotset_no_io() {
    let mut cargo_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cargo_dir_path.push("resources/benchmarks/mandelbrotset_no_io/");
    let mut wasm_path = cargo_dir_path.clone();
    wasm_path.push("mandelbrotset_no_io.wasm");
    let wasm_flags = WasiFlags::new(Some(()), None);
    let lambda = loader::build_lambda_from_file(
        &wasm_path,
        1024 * 1024 * 1000,
        Ipv4Addr::new(127, 0, 0, 1),
        wasm_flags,
    )
    .await
    .unwrap();
    lambda.run("").await.unwrap();
    assert!(true)
}
