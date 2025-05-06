use axum::{routing::post, Json, Router};
use clap::Parser;
use limes::runtime::runtime_error::RuntimeError;
use limes::runtime::{Runtime, RuntimeBuilder};
use tokio;
use tokio::net::TcpListener;

#[derive(Debug, Parser)]
pub struct ArgsParser {
    /// The ip address on which the server will listen, the default is localhost
    #[clap(short, long)]
    ip_address: Option<String>,
    /// The port to server to listen on
    #[clap(short, long)]
    port: Option<usize>,
    /// Number of cpu to use
    #[clap(short, long)]
    cpus: Option<usize>,
    /// The size of max memory limes will take
    #[clap(short, long)]
    memory: Option<usize>,
    /// Number of max function limes can deploy
    #[clap(short, long)]
    func_cap: Option<usize>,
}

#[tokio::main]
async fn main() {
    let args = ArgsParser::parse();
    let _runtime = match build_runtime(&args) {
        Ok(runtime) => runtime,
        Err(e) => panic!("{}", e.to_string()),
    };

    // Setup server
    // let router = Router::new().route(, );
}

#[allow(dead_code)]
async fn register_module() {
    todo!();
}

#[allow(dead_code)]
async fn remove_module() {
    todo!();
}

#[allow(dead_code)]
async fn init_function() {
    todo!();
}

#[allow(dead_code)]
async fn remove_function() {
    todo!();
}

#[allow(dead_code)]
async fn exec_function() {
    todo!();
}

#[allow(dead_code)]
async fn stop_function() {
    todo!();
}

fn build_runtime(args: &ArgsParser) -> anyhow::Result<Runtime> {
    let cpus = args.cpus.map_or(1, |cpus| cpus);
    let memory = args.memory.map_or(1024 * 1024 * 100, |memory| memory);
    let func_cap = args.func_cap.map_or(25, |func_cap| func_cap);

    // Create runtime
    let runtime = Runtime::new()
        .set_cpus(cpus)
        .set_total_memory_size(memory)
        .set_max_functions_number(func_cap)
        .build()?;

    Ok(runtime)
}
