use anyhow::Context;
use atoi::atoi;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use bytes::Bytes;
use clap::Parser;
use limes::runtime::Runtime;
use serde::{Deserialize, Serialize};
use std::{net::Ipv4Addr, str::FromStr, sync::Arc};

// --- DTOs (Data Transfer Objects) ---

#[derive(Deserialize)]
pub struct LoadFunctionPayload {
    pub function_memory_size: usize,
    pub tap_ip: Ipv4Addr,
    pub function_name: String,
    pub function_input_description: String,
    pub description: String,
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

// --- Handler Functions ---

async fn register_user_handler(
    State(runtime): State<Arc<Runtime>>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    match runtime.register_user().await {
        Ok(user_id) => Ok(Json(UserResponse { user_id })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
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
    body: Bytes, // Bytes of the wasm module directly inside the body
) -> Result<Json<ModuleResponse>, (StatusCode, String)> {
    match runtime.register_module(&user_id, &body).await {
        Ok(module_id) => Ok(Json(ModuleResponse { module_id })),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

async fn load_function_handler(
    State(runtime): State<Arc<Runtime>>,
    Path((user_id, module_id)): Path<(String, u32)>,
    Json(payload): Json<LoadFunctionPayload>,
) -> Result<Json<FunctionResponse>, (StatusCode, String)> {
    match runtime
        .load_function(
            &user_id,
            &module_id,
            payload.function_memory_size,
            payload.tap_ip,
            payload.function_name,
            payload.function_input_description,
            payload.description,
        )
        .await
    {
        Ok(function_id) => Ok(Json(FunctionResponse { function_id })),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

async fn exec_function_handler(
    State(runtime): State<Arc<Runtime>>,
    Path((user_id, function_id)): Path<(String, String)>,
    body: String, // Gli argomenti passati come stringa raw nel body
) -> Result<Json<ExecResponse>, (StatusCode, String)> {
    match runtime.exec_function(&user_id, &function_id, &body).await {
        Ok(result) => Ok(Json(ExecResponse { result })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// --- Setup Principale del Server ---

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// The ip address in the form of xxx.xxx.xxx.xxx
    #[arg(short, long)]
    ip: String,
    /// The port number in the form of xxxx
    #[arg(short, long)]
    port: String,
}

fn check_args(args: &Args) -> anyhow::Result<(Ipv4Addr, u32)> {
    let ip_addr = Ipv4Addr::from_str(&args.ip)
        .with_context(|| "There was an error with the given IP address")?;

    let port = atoi::<u32>(args.port.as_bytes())
        .with_context(|| "The inserted port is not a valid number")?;

    let port = if port > 1 && port < 65536 {
        port
    } else {
        return Err(anyhow::anyhow!("The inserted port is out of range"));
    };

    Ok((ip_addr, port))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (ip_address, port) = check_args(&args)?;

    // Per questo esempio simuliamo di avere già l'istanza:
    let _ = Runtime::new()
        .set_memory_size(1024 * 1024 * 100)
        .set_max_functions(100)
        .build()
        .await?;
    let runtime = Runtime::get_runtime_ref()?;

    // 2. Costruisci il Router Axum iniettando l'Arc<Runtime> come stato
    let app = Router::new()
        .route("/users", post(register_user_handler))
        .route("/users/:user_id", delete(remove_user_handler))
        .route("/users/:user_id/modules", post(register_module_handler))
        // .route("/users/:user_id/modules/:module_id", delete(remove_module_handler))
        .route(
            "/users/:user_id/modules/:module_id/functions",
            post(load_function_handler),
        )
        // .route("/users/:user_id/functions/:function_id", delete(unload_function_handler))
        // .route("/users/:user_id/functions/:function_id/stop", post(stop_function_handler))
        .route(
            "/users/:user_id/functions/:function_id/exec",
            post(exec_function_handler),
        )
        .with_state(runtime); // L'istanza clonata dell'Arc viene passata qui

    // 3. Avvia il server
    let listener = tokio::net::TcpListener::bind(&format!("{}:{}", ip_address, port)).await?;
    println!("Limes Server started on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
