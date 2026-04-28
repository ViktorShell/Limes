use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use async_trait::async_trait;

use crate::{
    agents::{LimesAgent, LimesTool},
    runtime::Runtime,
};
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
    pub function_description: String,
    pub function_input_json: Option<String>,
}

impl FunctionHandler {
    pub async fn new(
        component: Arc<Component>,
        user_id: String,
        function_id: String,
        function_name: String,
        function_description: String,
        function_input_json: Option<String>,
    ) -> anyhow::Result<Self> {
        let lambda = Lambda::new(component, &user_id).await?;
        Ok(Self {
            lambda,
            status: FunctionStatus::Ready,
            user_id,
            function_id,
            function_name,
            function_description,
            function_input_json,
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

#[derive(Debug)]
pub struct DynamicTool {
    pub func_handl: Arc<FunctionHandler>,
}

impl DynamicTool {
    pub fn new(func_handl: Arc<FunctionHandler>) -> Self {
        Self { func_handl }
    }
}

#[async_trait]
impl LimesTool for DynamicTool {
    fn name(&self) -> String {
        self.func_handl.function_name.clone()
    }

    fn description(&self) -> String {
        self.func_handl.function_description.clone()
    }

    fn parameters_json(&self) -> String {
        self.func_handl
            .function_input_json
            .clone()
            .unwrap_or_else(|| "{}".to_string())
    }

    async fn execute(&self, arguments: &str) -> anyhow::Result<String> {
        self.func_handl.lambda.run(arguments).await
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

        let mut agent = match LimesAgent::new().await {
            Ok(agent) => agent,
            Err(e) => {
                error!("Failed to initialize LimesAgent: {e}");
                return format!("AgentError: agent initialization failed - {e}");
            }
        };

        // Add tools
        let rt = Runtime::get_runtime_ref().unwrap();
        let user_funcs = rt
            .get_user_functions(&self.user_id)
            .await
            .unwrap_or_else(|_| vec![]);

        // Build Tools
        for func in user_funcs {
            let tool = Box::new(DynamicTool::new(func.clone()));
            if let Err(e) = agent.add_tool(tool) {
                info!("Failed to register the tool: {e}");
            }
        }

        agent
            .request(&input)
            .await
            .unwrap_or_else(|e| format!("Error: {e}"))
    }
}

pub struct Lambda {
    component: Arc<Component>,
    stop: Arc<AtomicBool>,
    user_id: String,
}

impl Lambda {
    pub async fn new(component: Arc<Component>, user_id: &str) -> anyhow::Result<Self> {
        info!("Lambda created");
        Ok(Self {
            component,
            stop: Arc::new(AtomicBool::new(false)),
            user_id: user_id.into(),
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
        let limiter = StoreLimitsBuilder::new().build();

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

    // ── Helpers ──

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

    async fn make_lambda(engine: &Engine, wasm_file: &str) -> Lambda {
        let path = PathBuf::from(format!("{WASM_DIR}/{wasm_file}"));
        let component = Arc::new(load_component(engine, &path));
        Lambda::new(component, "")
            .await
            .expect("Lambda::new failed")
    }

    // ── Tests ──

    #[tokio::test]
    async fn exec_single_lambda_function() {
        let engine = make_engine();
        let lambda = make_lambda(&engine, "exec_rust_lambda_function.wasm").await;
        let result = lambda.run("HELLO WORLD").await.unwrap();
        assert_eq!("HELLO WORLD### TEST ###", result);
    }

    #[tokio::test]
    async fn stop_infinite_loop_function() {
        let engine = make_engine();
        let lambda = Arc::new(make_lambda(&engine, "stop_infinite_loop.wasm").await);

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
        let lambda = Arc::new(make_lambda(&engine, "sorter.wasm").await);

        let (r1, r2) = tokio::join!(
            {
                let l = lambda.clone();
                tokio::spawn(async move { l.run(r#"{"items": "f,e,d,c,b,a"}"#).await })
            },
            {
                let l = lambda.clone();
                tokio::spawn(async move { l.run(r#"{"items": "e,d,c,b,a"}"#).await })
            }
        );

        assert_eq!(r#"{"content": "[a,b,c,d,e,f]"}"#, r1.unwrap().unwrap());
        assert_eq!(r#"{"content": "[a,b,c,d,e]"}"#, r2.unwrap().unwrap());
    }
}
