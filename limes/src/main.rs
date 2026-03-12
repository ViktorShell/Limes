use anyhow::{Context, Result};
use clap::Parser;
use limes::runtime::Runtime;
use std::fs::File;
use std::io::{BufReader, Read};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short = 'p', long)]
    file_path: PathBuf,
    #[arg(short, long)]
    func_arg: String,
    #[arg(short, long)]
    total_memory: usize,
    #[arg(short, long)]
    max_functions: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let wasm_bytes = load_bytes(&args.file_path)?;

    let rt = Runtime::new()
        .set_memory_size(args.total_memory)
        .set_max_functions(args.max_functions)
        .build()
        .await
        .unwrap();

    let user_id = rt
        .register_user()
        .await
        .context("Failed to register the user")?;
    let module_id = rt
        .register_module(&user_id, &wasm_bytes)
        .await
        .context("Failed to register the module")?;
    let function_id = rt
        .load_function(
            &user_id,
            &module_id,
            1024 * 1024 * 2,
            Ipv4Addr::new(127, 0, 0, 1),
            "func".to_string(),
            "no description".to_string(),
        )
        .await
        .context("Failed to load the function")?;

    let result = rt
        .exec_function(&user_id, &function_id, &args.func_arg)
        .await
        .context("Failed to execute the function")?;

    println!("Function output: {}", result);

    // Cleaning
    rt.unload_function(&user_id, &function_id)
        .await
        .context("Failed to unload the function")?;
    rt.remove_module(&user_id, &module_id)
        .await
        .context("Failed to unload the module")?;
    if !rt.remove_user(&user_id).await {
        return Err(anyhow::anyhow!("Failed to remove the user"));
    };

    Ok(())
}

fn load_bytes(path: &Path) -> anyhow::Result<Vec<u8>> {
    let file = File::open(path).context("No file found with path: {path}")?;
    let mut reader = BufReader::new(file);
    let mut buffer = vec![];
    reader
        .read_to_end(&mut buffer)
        .context("Could not read the file")?;
    Ok(buffer)
}
