use env_logger;
use limes::runtime::lambda;
use limes::tools::loader;
use log::info;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;
use std::{collections::HashMap, net::Ipv4Addr, time};
use tokio::runtime::Runtime;
use wasmtime_wasi::DirPerms;
use wasmtime_wasi::FilePerms;

fn main() {
    // Setup the logger
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    // Getting the cargo directory
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.push("resources/benchmarks");

    // End product csv
    let mut times_file = File::create("times.csv").expect("Could not create the times.csv");

    // Init header of the files
    writeln!(times_file, "iteration,func_name,elapsed_load,elapsed_mid")
        .expect("Could not write on the file");

    let iterations = 1;

    info!("Starting Cold Start evaluation");
    for iter in 0..iterations {
        let (name, start, mid, end) = evaluate_nop_cold_start(&mut root);
        let elapsed_load = (mid - start) as f32 / 1_000_000 as f32;
        let elapsed_exec = (end - mid) as f32 / 1_000_000 as f32;
        let data = format!("{},{},{},{}", iter, name, elapsed_load, elapsed_exec);
        writeln!(times_file, "{}", data).expect("Could not write on the file");
    }
    info!("Finish");

    info!("Starting img processing evaluation");
    for iter in 0..iterations {
        let (name, start, mid, end) = evaluate_img_proc(&mut root);
        let elapsed_load = (mid - start) as f32 / 1_000_000 as f32;
        let elapsed_exec = (end - mid) as f32 / 1_000_000 as f32;
        let data = format!("{},{},{},{}", iter, name, elapsed_load, elapsed_exec);
        writeln!(times_file, "{}", data).expect("Could not write on the file");
    }
    info!("Finish");

    info!("Starting img processing evaluation with no writes");
    for iter in 0..iterations {
        let (name, start, mid, end) = evaluate_img_proc_no_io(&mut root);
        let elapsed_load = (mid - start) as f32 / 1_000_000 as f32;
        let elapsed_exec = (end - mid) as f32 / 1_000_000 as f32;
        let data = format!("{},{},{},{}", iter, name, elapsed_load, elapsed_exec);
        writeln!(times_file, "{}", data).expect("Could not write on the file");
    }
    info!("Finish");

    info!("Starting mandelbrot set");
    for iter in 0..iterations {
        let (name, start, mid, end) = evaluate_mandelbrotset(&mut root);
        let elapsed_load = (mid - start) as f32 / 1_000_000 as f32;
        let elapsed_exec = (end - mid) as f32 / 1_000_000 as f32;
        let data = format!("{},{},{},{}", iter, name, elapsed_load, elapsed_exec);
        writeln!(times_file, "{}", data).expect("Could not write on the file");
    }
    info!("Finish");

    info!("Starting mandelbrot set with no writes");
    for iter in 0..iterations {
        let (name, start, mid, end) = evaluate_mandelbrotset_no_io(&mut root);
        let elapsed_load = (mid - start) as f32 / 1_000_000 as f32;
        let elapsed_exec = (end - mid) as f32 / 1_000_000 as f32;
        let data = format!("{},{},{},{}", iter, name, elapsed_load, elapsed_exec);
        writeln!(times_file, "{}", data).expect("Could not write on the file");
    }
    info!("Finish");
    info!("End of the process");
}

fn evaluate_img_proc(root: &mut PathBuf) -> (String, u128, u128, u128) {
    // Get file
    let mut file = root.clone();
    file.push("limes_img_processing/limes_img_processing.wasm");
    let mut img_dir = root.clone();
    img_dir.push("limes_img_processing/images");

    // WasiFlags
    let mut file_map: HashMap<String, (String, DirPerms, FilePerms)> = HashMap::new();
    file_map.insert(
        img_dir
            .to_str()
            .expect("Could not conver img dir")
            .to_string(),
        ("./".to_string(), DirPerms::all(), FilePerms::all()),
    );

    // Function init
    let wasi_flags = lambda::WasiFlags::new(Some(()), Some(file_map));

    // Runtime for sync execution on Tokio
    let rt = Runtime::new().expect("Error when setting up the runtime");

    let time_start = get_time();

    let lambda = rt.block_on(async {
        loader::build_lambda(
            &file,
            1024 * 1024 * 500,
            Ipv4Addr::new(127, 0, 0, 1),
            wasi_flags,
        )
        .await
        .expect("Error on init of img proc")
    });

    let time_mid = get_time();
    let _ = rt.block_on(async { lambda.run("").await.expect("Error executing img proc") });
    let time_end = get_time();

    // Return result
    (
        "image_processing".to_string(),
        time_start,
        time_mid,
        time_end,
    )
}

fn evaluate_img_proc_no_io(root: &mut PathBuf) -> (String, u128, u128, u128) {
    // Get file
    let mut file = root.clone();
    file.push("limes_img_processing_no_io/limes_img_processing_no_io.wasm");
    let mut img_dir = root.clone();
    img_dir.push("limes_img_processing_no_io/images");

    // WasiFlags
    let mut file_map: HashMap<String, (String, DirPerms, FilePerms)> = HashMap::new();
    file_map.insert(
        img_dir
            .to_str()
            .expect("Could not conver img dir")
            .to_string(),
        ("./".to_string(), DirPerms::all(), FilePerms::all()),
    );

    // Function init
    let wasi_flags = lambda::WasiFlags::new(Some(()), Some(file_map));

    // Runtime for sync execution on Tokio
    let rt = Runtime::new().expect("Error when setting up the runtime");

    let time_start = get_time();

    let lambda = rt.block_on(async {
        loader::build_lambda(
            &file,
            1024 * 1024 * 500,
            Ipv4Addr::new(127, 0, 0, 1),
            wasi_flags,
        )
        .await
        .expect("Error on init of img proc")
    });

    let time_mid = get_time();
    let _ = rt.block_on(async { lambda.run("").await.expect("Error executing img proc") });
    let time_end = get_time();

    // Return result
    (
        "image_processing_no_io".to_string(),
        time_start,
        time_mid,
        time_end,
    )
}

fn evaluate_mandelbrotset(root: &mut PathBuf) -> (String, u128, u128, u128) {
    // Get file
    let mut file = root.clone();
    file.push("mandelbrotset/mandelbrotset.wasm");
    let mut img_dir = root.clone();
    img_dir.push("mandelbrotset/images");

    // WasiFlags
    let mut file_map: HashMap<String, (String, DirPerms, FilePerms)> = HashMap::new();
    file_map.insert(
        img_dir
            .to_str()
            .expect("Could not conver img dir")
            .to_string(),
        ("./".to_string(), DirPerms::all(), FilePerms::all()),
    );

    // Function init
    let wasi_flags = lambda::WasiFlags::new(Some(()), Some(file_map));

    // Runtime for sync execution on Tokio
    let rt = Runtime::new().expect("Error when setting up the runtime");

    let time_start = get_time();

    let lambda = rt.block_on(async {
        loader::build_lambda(
            &file,
            1024 * 1024 * 500,
            Ipv4Addr::new(127, 0, 0, 1),
            wasi_flags,
        )
        .await
        .expect("Error on init of img proc")
    });

    let time_mid = get_time();
    let _ = rt.block_on(async { lambda.run("").await.expect("Error executing img proc") });
    let time_end = get_time();

    // Return result
    ("mandelbrotset".to_string(), time_start, time_mid, time_end)
}

fn evaluate_mandelbrotset_no_io(root: &mut PathBuf) -> (String, u128, u128, u128) {
    // Get file
    let mut file = root.clone();
    file.push("mandelbrotset_no_io/mandelbrotset_no_io.wasm");

    // WasiFlags
    let wasi_flags = lambda::WasiFlags::new(Some(()), None);

    // Runtime for sync execution on Tokio
    let rt = Runtime::new().expect("Error when setting up the runtime");

    let time_start = get_time();

    let lambda = rt.block_on(async {
        loader::build_lambda(
            &file,
            1024 * 1024 * 500,
            Ipv4Addr::new(127, 0, 0, 1),
            wasi_flags,
        )
        .await
        .expect("Error on init of img proc")
    });

    let time_mid = get_time();
    let _ = rt.block_on(async { lambda.run("").await.expect("Error executing img proc") });
    let time_end = get_time();

    // Return result
    (
        "mandelbrotset_no_io".to_string(),
        time_start,
        time_mid,
        time_end,
    )
}

fn evaluate_nop_cold_start(root: &mut PathBuf) -> (String, u128, u128, u128) {
    // Get file
    let mut file = root.clone();
    file.push("nop_cold_start/nop_cold_start.wasm");

    // WasiFlags
    let wasi_flags = lambda::WasiFlags::new(Some(()), None);

    // Runtime for sync execution on Tokio
    let rt = Runtime::new().expect("Error when setting up the runtime");

    let time_start = get_time();

    let lambda = rt.block_on(async {
        loader::build_lambda(
            &file,
            1024 * 1024 * 500,
            Ipv4Addr::new(127, 0, 0, 1),
            wasi_flags,
        )
        .await
        .expect("Error on init of img proc")
    });

    let time_mid = get_time();
    let _ = rt.block_on(async { lambda.run("").await.expect("Error executing img proc") });
    let time_end = get_time();

    // Return result
    ("nop_cold_start".to_string(), time_start, time_mid, time_end)
}

fn get_time() -> u128 {
    time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Adjust the clock over 1970...")
        .as_nanos()
}
