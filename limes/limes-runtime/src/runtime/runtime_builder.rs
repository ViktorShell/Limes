use super::*;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RuntimeBuilder {
    pub memory_size: Option<usize>,
    pub max_functions: Option<usize>,
}

impl RuntimeBuilder {
    pub fn set_memory_size(&mut self, memory_size: usize) -> &mut Self {
        self.memory_size = Some(memory_size);
        self
    }

    pub fn set_max_functions(&mut self, max_functions: usize) -> &mut Self {
        self.max_functions = Some(max_functions);
        self
    }

    /// Will build the wasmtime engine and configure the Runtime
    pub async fn build(&self) -> anyhow::Result<Arc<Runtime>> {
        let engine = Engine::new(
            Config::new()
                .async_support(true)
                .wasm_component_model(true)
                .epoch_interruption(true)
                .cranelift_opt_level(wasmtime::OptLevel::SpeedAndSize),
        )
        .with_context(|| "Runtime: Failed to build the Wasmtime Engine")?;

        // Check if is already setted
        let runtime_arc = Arc::new(Runtime {
            memory_size: self.memory_size.unwrap_or(1024 * 1024 * 10),
            max_allocatable_functions: self.max_functions.unwrap_or(100),
            currently_allocated_functions: Arc::new(AtomicUsize::new(0)),
            wasm_engine: Arc::new(engine),
            users: Arc::new(RwLock::new(HashMap::new())),
        });

        // Needed for self reference for the Agents
        let _ = RUNTIME_REF.set(runtime_arc.clone());

        Ok(runtime_arc)
    }
}
