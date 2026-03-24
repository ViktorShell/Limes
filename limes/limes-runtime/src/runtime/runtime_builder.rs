use log::*;
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::RwLock;
use wasmtime::{Config, Engine};

use super::Runtime;

pub struct RuntimeBuilder {
    pub memory_size: usize,
    pub max_functions: usize,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self {
            memory_size: 1024 * 1024 * 100, // 100 MiB
            max_functions: 100,
        }
    }
}

impl RuntimeBuilder {
    pub fn memory_size(&mut self, bytes: usize) -> &mut Self {
        self.memory_size = bytes;
        self
    }

    pub fn set_memory_size(&mut self, memory_size: usize) -> &mut Self {
        self.memory_size = memory_size;
        self
    }

    pub fn set_max_functions(&mut self, max_functions: usize) -> &mut Self {
        self.max_functions = max_functions;
        self
    }

    pub async fn build(&mut self) -> anyhow::Result<Arc<Runtime>> {
        let engine = Engine::new(
            Config::new()
                .wasm_component_model(true)
                .epoch_interruption(true)
                .cranelift_opt_level(wasmtime::OptLevel::SpeedAndSize),
        )
        .map_err(|e| {
            anyhow::anyhow!("RuntimeBuilder: failed to initialize the Wasmtime engine: {e}")
        })?;

        let runtime = Arc::new(Runtime {
            memory_size: self.memory_size,
            max_allocatable_functions: self.max_functions,
            currently_allocated_functions: Arc::new(AtomicUsize::new(0)),
            wasm_engine: Arc::new(engine),
            users: Arc::new(RwLock::new(HashMap::new())),
        });

        // Store the global reference (ignore error when called from tests).
        Runtime::set_global(runtime.clone());

        info!(
            r#"
Limes Runtime initialized:
>> memory_size: {}
>> max_functions: {}
        "#,
            self.memory_size, self.max_functions
        );

        Ok(runtime)
    }
}
