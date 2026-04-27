use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::agents::LimesAgent;
use log::*;
use thiserror::Error;
use wasmtime::{
    component::{bindgen, Component, HasSelf, Linker, ResourceTable},
    Store, StoreLimits, StoreLimitsBuilder,
};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

#[derive(Error, Debug, PartialEq, PartialOrd)]
pub enum LambdaError {
    #[error("Function execution was force-stopped via epoch interruption")]
    ForceStop,
    #[error("Function raised a Wasm trap or internal error during execution")]
    FunctionExecError,
    #[error("Cannot stop a function that is not currently running")]
    FunctionNotRunning,
}

#[derive(Debug)]
pub enum FunctionStatus {
    Ready,
    Running,
    Stopped,
}

/// Wraps a `Lambda` with metadata and lifecycle state for the runtime.
#[derive(Debug)]
pub struct FunctionHandler {
    pub lambda: Lambda,
    pub status: FunctionStatus,
    pub user_id: String,
    pub function_id: String,
    pub function_name: String,
    pub function_input_description: String,
    pub description: String,
}

impl FunctionHandler {
    pub async fn new(
        component: Arc<Component>,
        memory_size: usize,
        function_name: String,
        function_input_description: String,
        description: String,
        user_id: String,
        function_id: String,
    ) -> anyhow::Result<Self> {
        let lambda = Lambda::new(component, memory_size, user_id.clone()).await?;
        Ok(Self {
            lambda,
            status: FunctionStatus::Ready,
            function_name,
            function_input_description,
            description,
            user_id,
            function_id,
        })
    }
}

// WIT generator
bindgen!({
    inline: r#"
        package limes:limes;
        world executor {
            import invoke-agent: func(input: string) -> string;
            export run: func(input: string) -> string;
        }
    "#,
    imports: {default: async},
    exports: {default: async},
});

pub struct LambdaState {
    wasi_ctx: WasiCtx,
    resource_table: ResourceTable,
    limiter: StoreLimits,
    user_id: String,
}

impl WasiView for LambdaState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.resource_table,
        }
    }
}

// WIT implementation, it actually generates the Traits usable by the Guest machine
impl ExecutorImports for LambdaState {
    async fn invoke_agent(&mut self, input: String) -> String {
        debug!(
            r#"
            Guest invoked the agent:
                user_id: {}"#,
            self.user_id
        );

        let agent = match LimesAgent::new_agent(
            "http://127.0.0.1:11434",
            "llama3.2:3b",
            self.user_id.clone(),
        )
        .await
        {
            Ok(agent) => agent,
            Err(e) => {
                error!("Failed to initialize LimesAgent: {e}");
                return format!("AgentError: agent initialization failed - {e}");
            }
        };

        match agent.invoke_agent(input).await {
            Ok(answer) => {
                debug!("Agent responded successfully");
                answer
            }
            Err(e) => {
                error!("Agent invocation failed: {e}");
                format!("AgentError: agent invocation failed - {e}")
            }
        }
    }
}

pub struct Lambda {
    component: Arc<Component>,
    memory_size: usize,
    stop: Arc<AtomicBool>,
    user_id: String,
}

impl Lambda {
    const MIN_MEMORY_BYTES: usize = 1024 * 1024 * 2; // 2 MiB

    pub async fn new(
        component: Arc<Component>,
        memory_size: usize,
        user_id: String,
    ) -> anyhow::Result<Self> {
        if memory_size < Self::MIN_MEMORY_BYTES {
            return Err(anyhow::anyhow!(
                "Lambda: requested memory {} bytes is below the minimum {} bytes",
                memory_size,
                Self::MIN_MEMORY_BYTES
            ));
        }
        info!("Lambda created");
        Ok(Self {
            component,
            memory_size,
            stop: Arc::new(AtomicBool::new(false)),
            user_id,
        })
    }

    pub async fn run(&self, args: &str) -> anyhow::Result<String> {
        debug!("Lambda::run invoked");

        let engine = self.component.engine();
        let mut linker = Linker::<LambdaState>::new(engine);

        // Register the standard WASI host functions.
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;

        // Linker now can interact with invoke-agent
        Executor::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;

        // Store for memory
        let mut store = self.build_store();
        self.install_epoch_callback(&mut store);

        // Binding to the run function of the guest
        let bindings = Executor::instantiate_async(&mut store, &self.component, &linker).await?;

        // Exec the functions
        let result = bindings.call_run(&mut store, args).await.map_err(|e| {
            if self.stop.load(Ordering::SeqCst) {
                warn!("Lambda execution was force-stopped: {e}");
                LambdaError::ForceStop
            } else {
                error!("Lambda execution error: {e}");
                LambdaError::FunctionExecError
            }
        })?;

        debug!("Lambda::run completed successfully");
        Ok(result)
    }

    // Signal the running Wasm instance to stop via epoch interruption.
    pub async fn stop(&self) -> anyhow::Result<()> {
        if self.stop.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!(LambdaError::FunctionNotRunning));
        }
        self.stop.store(true, Ordering::SeqCst);
        self.component.engine().increment_epoch();
        info!("Lambda stop signal received");
        Ok(())
    }

    fn build_store(&self) -> Store<LambdaState> {
        let limiter = StoreLimitsBuilder::new()
            .memory_size(self.memory_size)
            .build();

        let state = LambdaState {
            wasi_ctx: WasiCtxBuilder::new().inherit_network().build(),
            resource_table: ResourceTable::new(),
            limiter,
            user_id: self.user_id.clone(),
        };

        let mut store = Store::new(self.component.engine(), state);
        store.limiter(|data| &mut data.limiter);
        store
    }

    /// Installs an epoch-interruption callback that allows a cooperative stop.
    fn install_epoch_callback(&self, store: &mut Store<LambdaState>) {
        let stop_flag = self.stop.clone();
        store.epoch_deadline_callback(move |_| {
            if stop_flag.load(Ordering::SeqCst) {
                Err(wasmtime::Error::msg(
                    "Lambda: epoch deadline — force stop requested",
                ))
            } else {
                Ok(wasmtime::UpdateDeadline::Yield(1))
            }
        });
    }
}

impl std::fmt::Debug for Lambda {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lambda")
            .field("memory_size", &self.memory_size)
            .field("user_id", &self.user_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{Lambda, LambdaError};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use wasmtime::{component::Component, Config, Engine, OptLevel};

    static WASM_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/resources/lamda_tests/wasm_compiled"
    );

    // ── Helpers ────────────────────────────────────────────────────────────

    fn make_engine() -> Engine {
        let mut cfg = Config::new();
        cfg.wasm_component_model(true)
            .epoch_interruption(true)
            .cranelift_opt_level(OptLevel::SpeedAndSize);
        Engine::new(&cfg).expect("Failed to build test engine")
    }

    fn load_component(engine: &Engine, path: &Path) -> Component {
        Component::from_file(engine, path).expect("Wasm component not found")
    }

    async fn make_lambda(engine: &Engine, wasm_file: &str, memory: usize) -> Lambda {
        let path = PathBuf::from(format!("{WASM_DIR}/{wasm_file}"));
        let component = Arc::new(load_component(engine, &path));
        Lambda::new(component, memory, String::new())
            .await
            .expect("Lambda::new failed")
    }

    const MEM_2MIB: usize = 1024 * 1024 * 2;

    // ── Tests ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn exec_single_lambda_function() {
        let engine = make_engine();
        let lambda = make_lambda(&engine, "exec_rust_lambda_function.wasm", MEM_2MIB).await;
        let result = lambda.run("HELLO WORLD").await.unwrap();
        assert_eq!("HELLO WORLD### TEST ###", result);
    }

    #[tokio::test]
    async fn stop_infinite_loop_function() {
        let engine = make_engine();
        let lambda = Arc::new(make_lambda(&engine, "stop_infinite_loop.wasm", MEM_2MIB).await);

        let run_handle = {
            let l = lambda.clone();
            tokio::spawn(async move { l.run("").await })
        };

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        lambda.stop().await.expect("stop() failed");

        let result = run_handle.await.expect("task panicked");
        let err = result.expect_err("Expected LambdaError::ForceStop but run() succeeded");
        let lambda_err = err
            .downcast_ref::<LambdaError>()
            .expect("downcast to LambdaError failed");
        assert_eq!(*lambda_err, LambdaError::ForceStop);
    }

    #[tokio::test]
    async fn multiple_function_execution() {
        let engine = make_engine();
        let lambda = Arc::new(make_lambda(&engine, "sorter.wasm", MEM_2MIB).await);

        let (r1, r2) = tokio::join!(
            {
                let l = lambda.clone();
                tokio::spawn(async move { l.run("f,e,d,c,b,a").await })
            },
            {
                let l = lambda.clone();
                tokio::spawn(async move { l.run("e,d,c,b,a").await })
            }
        );

        assert_eq!("[a,b,c,d,e,f]", r1.unwrap().unwrap());
        assert_eq!("[a,b,c,d,e]", r2.unwrap().unwrap());
    }
}
