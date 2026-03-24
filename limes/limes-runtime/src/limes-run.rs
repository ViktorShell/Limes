use anyhow::{Context, Result};
use clap::Parser;
use limes::runtime::Runtime;
use log::info;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────────
//  CLI
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    version,
    about = "limes-run — execute a single Wasm module and print the result"
)]
struct Args {
    /// Path to the .wasm component file
    #[arg(short = 'p', long)]
    file_path: PathBuf,

    /// Argument string passed to the Wasm `run` function
    #[arg(short, long)]
    func_arg: String,

    /// Memory budget for the function (bytes)
    #[arg(short, long, default_value_t = 1024 * 1024 * 2)]
    memory: usize,

    /// Maximum number of functions (unused for single-run, kept for API compat)
    #[arg(long, default_value_t = 10)]
    max_functions: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
//  Entry point
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    let wasm_bytes = load_bytes(&args.file_path)?;

    let rt = Runtime::runtime_builder()
        .set_memory_size(args.memory * 2 + 1024 * 1024 * 10) // a bit more than the function
        .set_max_functions(args.max_functions)
        .build()
        .await
        .context("Failed to initialize the runtime")?;

    let user_id = rt
        .register_user()
        .await
        .context("Failed to register user")?;

    let module_id = rt
        .register_module(&user_id, &wasm_bytes)
        .await
        .context("Failed to register the Wasm module")?;

    let function_id = rt
        .load_function(
            &user_id,
            &module_id,
            args.memory,
            "function".into(),
            "raw string".into(),
            "executed via limes-run".into(),
        )
        .await
        .context("Failed to load the function")?;

    let result = rt
        .exec_function(&user_id, &function_id, &args.func_arg)
        .await
        .context("Failed to execute the function")?;

    info!("Function execution completed with result: {result}");

    // Cleanup
    rt.unload_function(&user_id, &function_id)
        .await
        .context("Failed to unload function")?;
    rt.remove_module(&user_id, &module_id)
        .await
        .context("Failed to remove module")?;
    rt.remove_user(&user_id).await;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn load_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("Could not read file: {}", path.display()))
}
