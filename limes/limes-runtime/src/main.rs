use anyhow::Result;
use log::*;

use base64::{engine::general_purpose, Engine as _};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, post},
    Json, Router,
};
use clap::Parser;
use limes::{config::Config, runtime::Runtime};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{env, sync::Arc};

// DTO's
#[derive(Deserialize)]
pub struct RegisterModulePayload {
    pub wasm_base64: String,
    pub function_name: String,
    pub function_description: String,
    pub function_input_json: Option<String>,
}

#[derive(Deserialize)]
pub struct FunctionExecBody {
    pub arguments: String,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct ModuleResponse {
    pub module_id: u32,
}

#[derive(Serialize)]
pub struct FunctionResponse {
    pub function_id: String,
}

#[derive(Serialize)]
pub struct ExecResponse {
    pub result: String,
}

// API hanlders
async fn register_user_handler(
    State(runtime): State<Arc<Runtime>>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    runtime
        .register_user()
        .await
        .map(|user_id| Json(UserResponse { user_id }))
        .map_err(internal_error)
}

async fn remove_user_handler(
    State(runtime): State<Arc<Runtime>>,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    if runtime.remove_user(&user_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn register_module_handler(
    State(runtime): State<Arc<Runtime>>,
    Path(user_id): Path<String>,
    Json(payload): Json<RegisterModulePayload>,
) -> Result<Json<ModuleResponse>, (StatusCode, String)> {
    let wasm_bytes = general_purpose::STANDARD
        .decode(&payload.wasm_base64)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid base64: {e}")))?;

    runtime
        .register_module(
            &user_id,
            &wasm_bytes,
            payload.function_name,
            payload.function_description,
            payload.function_input_json,
        )
        .await
        .map(|module_id| Json(ModuleResponse { module_id }))
        .map_err(|e| {
            warn!("register_module failed: {e}");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to register the module: {e}"),
            )
        })
}

async fn load_function_handler(
    State(runtime): State<Arc<Runtime>>,
    Path((user_id, module_id)): Path<(String, u32)>,
) -> Result<Json<FunctionResponse>, (StatusCode, String)> {
    runtime
        .load_function(&user_id, &module_id)
        .await
        .map(|function_id| Json(FunctionResponse { function_id }))
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

async fn exec_function_handler(
    State(runtime): State<Arc<Runtime>>,
    Path((user_id, function_id)): Path<(String, String)>,
    Json(payload): Json<FunctionExecBody>,
) -> Result<Json<ExecResponse>, (StatusCode, String)> {
    runtime
        .exec_function(&user_id, &function_id, &payload.arguments)
        .await
        .map(|result| Json(ExecResponse { result }))
        .map_err(|e| {
            error!("exec_function failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })
}

// CLI
#[derive(Parser, Debug)]
#[command(
    version,
    about = "Limes — FaaS runtime for Wasm modules with LLM support"
)]
pub struct Args {
    /// Server bind address (e.g. 127.0.0.1)
    #[arg(short, long, default_value = "127.0.0.1")]
    ip: String,

    /// Server port
    #[arg(short, long, default_value = "50500")]
    port: u16,

    /// Total runtime memory budget in bytes
    #[arg(long, default_value_t = 1024 * 1024 * 100)]
    memory: usize,

    /// Maximum number of concurrently loaded functions
    #[arg(long, default_value_t = 100)]
    max_functions: usize,

    /// Ollama url
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    ollama_url: String,

    /// LLM model with thinking ability
    #[arg(long, default_value = "qwen3:8b-q4_K_M ")]
    llm_model: String,

    /// Timemout of a request to an LLM in seconds
    #[arg(long, default_value_t = 250)]
    request_timeout: u64,

    /// Verbose, show information about the execution status
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Init logger
    if args.verbose {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    // Init Global Config
    Config::init(Config {
        ollama_url: args.ollama_url,
        model: args.llm_model,
        request_timeout: Duration::from_secs(args.request_timeout),
    });

    Runtime::runtime_builder()
        .memory_size(args.memory)
        .set_max_functions(args.max_functions)
        .build()
        .await?;

    let runtime_ref = Runtime::get_runtime_ref().map_err(|e| anyhow::anyhow!("{e}"))?;

    let app = Router::new()
        .route("/users", post(register_user_handler))
        .route("/users/:user_id", delete(remove_user_handler))
        .route("/users/:user_id/modules", post(register_module_handler))
        .route(
            "/users/:user_id/modules/:module_id/functions",
            post(load_function_handler),
        )
        .route(
            "/users/:user_id/functions/:function_id/exec",
            post(exec_function_handler),
        )
        .with_state(runtime_ref);

    let addr = format!("{}:{}", args.ip, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Logo
    println!(
        r#"
              _      _                     
             | |    (_)                    
             | |     _ _ __ ___   ___  ___ 
             | |    | | '_ ` _ \ / _ \/ __|
             | |____| | | | | | |  __/\__ \
             |______|_|_| |_| |_|\___||___/
                                           

            Limes server started on ip: {}"#,
        listener.local_addr()?
    );

    axum::serve(listener, app).await?;
    Ok(())
}

// Helper
fn internal_error(e: anyhow::Error) -> (StatusCode, String) {
    error!("Internal server error: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}
