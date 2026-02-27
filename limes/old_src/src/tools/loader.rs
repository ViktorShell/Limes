use crate::runtime::lambda::Lambda;
use crate::runtime::lambda::WasiFlags;
use crate::runtime::lambda_error::LambdaError;
use anyhow::Result;
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Arc;
use wasmtime::component::Component;
use wasmtime::Config;
use wasmtime::Engine;
use wasmtime::OptLevel;

pub async fn load_module_from_file(engine: &Engine, file: &Path) -> Result<Arc<Component>> {
    Ok(Arc::new(Component::from_file(engine, file)?))
}

pub async fn load_module_from_bytes(engine: &Engine, image: &[u8]) -> Result<Arc<Component>> {
    Ok(Arc::new(Component::from_binary(engine, image)?))
}

pub async fn build_engine(async_support: bool, wasm_component_module: bool) -> Result<Engine> {
    let mut config = Config::new();
    config
        .async_support(async_support)
        .wasm_component_model(wasm_component_module)
        .cranelift_opt_level(OptLevel::SpeedAndSize);
    let engine = Engine::new(&config)?;
    Ok(engine)
}

pub async fn build_lambda_from_file(
    file: &Path,
    mem_size: usize,
    tap_ip: Ipv4Addr,
    wasi_flags: WasiFlags,
) -> Result<Lambda> {
    let engine = build_engine(true, true).await?;
    let component = load_module_from_file(&engine, file).await?;
    let lambda = Lambda::new(component, mem_size, tap_ip, wasi_flags).await?;
    Ok(lambda)
}
