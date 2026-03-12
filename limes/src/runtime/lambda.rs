use std::{
    future::Future,
    net::{Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::Context;
use thiserror::Error;

use wasmtime::Store;
use wasmtime::StoreLimits;
use wasmtime::{
    component::{Component, Instance, Linker, ResourceTable, TypedFunc},
    StoreLimitsBuilder,
};
use wasmtime_wasi::{IoView, SocketAddrUse, WasiCtx, WasiCtxBuilder, WasiView};

#[derive(PartialEq, PartialOrd, Debug)]
pub enum FunctionStatus {
    Ready,
    Running,
    Stopped,
}

/// The Function Handler have the role to create and handle the lambda functions
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
        tap_ip: Ipv4Addr,
        function_name: String,
        function_input_description: String,
        description: String,
        user_id: String,
        function_id: String,
    ) -> anyhow::Result<Self> {
        let lambda = Lambda::new(component, memory_size, tap_ip).await?;
        let status = FunctionStatus::Ready;
        Ok(Self {
            lambda,
            status,
            function_name,
            function_input_description,
            description,
            user_id,
            function_id,
        })
    }
}

pub struct LambdaState {
    wasi_ctx: WasiCtx,
    resource_table: ResourceTable,
    limiter: StoreLimits,
}

impl IoView for LambdaState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.resource_table
    }
}

impl WasiView for LambdaState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }
}

#[derive(Error, Debug, PartialEq, PartialOrd)]
pub enum LambdaError {
    #[error("The function execution was stopped")]
    ForceStop,
    #[error("The function generated and execution error")]
    FunctionExecError,
    #[error("The function is not running")]
    FunctionNotRunning,
}

pub struct Lambda {
    component: Arc<Component>,
    memory_size: usize,
    tap_ip: Ipv4Addr,
    stop: Arc<AtomicBool>,
}

impl Lambda {
    pub async fn new(
        component: Arc<Component>,
        memory_size: usize,
        tap_ip: Ipv4Addr,
    ) -> anyhow::Result<Lambda> {
        if memory_size < 1024 * 1024 * 2 {
            return Err(anyhow::anyhow!(
                "Lambda initialization: Not enought memory: {}",
                memory_size
            ));
        }
        let stop = Arc::new(AtomicBool::new(false));
        Ok(Self {
            component,
            memory_size,
            tap_ip,
            stop,
        })
    }

    pub async fn run(&self, args: &str) -> anyhow::Result<String> {
        // Init the linker with wasi
        let engine = self.component.engine();
        let mut linker = Linker::<LambdaState>::new(engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        // Build the context for function execution
        let wasi_ctx = self.get_wasictx();
        let mut store = self.get_store(wasi_ctx);

        // Interrupt mechanism
        self.init_interrupt_callback(&mut store);

        // Get the function Instance from Component
        let instance = linker
            .instantiate_async(&mut store, &self.component)
            .await
            .with_context(|| "Lambda Error: Unable to load the instance of the component")?;

        // Retriev the function
        let func = self.get_main_func(&instance, &mut store)?;

        // Exec main function
        let result = func
            .call_async(&mut store, (args,))
            .await
            .map_err(|_| match self.stop.load(Ordering::Relaxed) {
                true => LambdaError::ForceStop,
                false => LambdaError::FunctionExecError,
            })?
            .0;

        // Reset the store even though it will be de-allocated.
        // I will remove it soon and change the way the function exec.
        // let _ = func.post_return_async(&mut store).await;
        Ok(result)
    }

    // pub async fn async_run_tusk(
    //     &self,
    //     args: &str,
    // ) -> Box<
    //     dyn Fn(
    //         String,
    //     )
    //         -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send> + Sync + Send>,
    // > {
    //     // Init the linker with wasi
    //     let engine = self.component.engine();
    //     let mut linker = Linker::<LambdaState>::new(engine);
    //     wasmtime_wasi::add_to_linker_async(&mut linker)?;
    //
    //     // Build the context for function execution
    //     let wasi_ctx = self.get_wasictx();
    //     let mut store = self.get_store(wasi_ctx);
    //
    //     // Interrupt mechanism
    //     self.init_interrupt_callback(&mut store);
    //
    //     // Get the function Instance from Component
    //     let instance = linker
    //         .instantiate_async(&mut store, &self.component)
    //         .await
    //         .with_context(|| "Lambda Error: Unable to load the instance of the component")?;
    //
    //     // Retriev the function
    //     let func = self.get_main_func(&instance, &mut store)?;
    //
    //     // Get the function Instance from Component
    //     let instance = linker
    //         .instantiate_async(&mut store, &self.component)
    //         .await
    //         .with_context(|| "Lambda Error: Unable to load the instance of the component")?;
    //
    //     let result = Box::new(|args: String| {
    //         Box::pin(async move {
    //             let result = func.call_async(&mut store, (&args,)).await.map_err(|_| {
    //                 match self.stop.load(Ordering::Relaxed) {
    //                     true => LambdaError::ForceStop,
    //                     false => LambdaError::FunctionExecError,
    //                 }
    //             });
    //             result
    //                 .map(|(s,)| s)
    //                 .context("Agent: Failed to execute the function")
    //         })
    //     });
    //
    //     result
    // }

    pub async fn stop(&self) -> anyhow::Result<()> {
        let engine = self.component.engine();
        if self.stop.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!(LambdaError::FunctionNotRunning));
        }
        self.stop.store(true, Ordering::SeqCst);
        engine.increment_epoch();
        Ok(())
    }

    fn get_main_func(
        &self,
        instance: &Instance,
        store: &mut Store<LambdaState>,
    ) -> anyhow::Result<TypedFunc<(&str,), (String,)>> {
        let interface_idx = instance
            .get_export(&mut *store, None, "component:run/run")
            .ok_or(anyhow::anyhow!("Function Interface Error"))?;

        let func_idx = instance
            .get_export(&mut *store, Some(&interface_idx), "run")
            .ok_or(anyhow::anyhow!("Didn't find the component:run/run -> run"))?;

        instance
            .get_typed_func::<(&str,), (String,)>(store, func_idx)
            .with_context(|| "Function Retriev Error")
    }

    fn init_interrupt_callback(&self, store: &mut Store<LambdaState>) {
        let stop = self.stop.clone();
        store.epoch_deadline_callback(move |_| {
            if !stop.load(Ordering::SeqCst) {
                return Ok(wasmtime::UpdateDeadline::Yield(1));
            }
            Err(anyhow::anyhow!("ForceStop"))
        });
    }

    fn get_store(&self, wasi_ctx: WasiCtx) -> Store<LambdaState> {
        let resource = ResourceTable::new();
        let store_limits = StoreLimitsBuilder::new()
            .memory_size(self.memory_size)
            .build();
        let state = LambdaState {
            wasi_ctx,
            resource_table: resource,
            limiter: store_limits,
        };
        let mut store = Store::new(self.component.engine(), state);
        store.limiter(|data| &mut data.limiter);
        store
    }

    fn get_wasictx(&self) -> WasiCtx {
        // Ip connection filter
        let tap_ip = self.tap_ip;
        let f_socket_check =
            move |s_addr: SocketAddr,
                  _usage: SocketAddrUse|
                  -> Pin<Box<dyn Future<Output = bool> + Send + Sync + 'static>> {
                Box::pin(async move {
                    match _usage {
                        SocketAddrUse::TcpBind | SocketAddrUse::UdpBind => match s_addr {
                            SocketAddr::V4(addr_v4) => addr_v4.ip().eq(&tap_ip),
                            SocketAddr::V6(_) => false,
                        },
                        _ => true,
                    }
                })
            };

        let mut wasictx = WasiCtxBuilder::new();
        wasictx
            .inherit_network()
            .socket_addr_check(f_socket_check)
            .build()
    }
}

impl std::fmt::Debug for Lambda {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lambda")
    }
}

#[cfg(test)]
mod test {
    use crate::runtime::lambda::{Lambda, LambdaError};
    use std::env;
    use std::net::Ipv4Addr;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tokio;
    use wasmtime::component::Component;
    use wasmtime::*;

    // NOTE: Directory where the wasm functions are located
    static WASM_RESOURCES: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/resources/lamda_tests/wasm_compiled"
    );

    // NOTE: Actual Tests

    #[tokio::test]
    async fn exec_single_lambda_function() {
        let engine = gen_engine();
        let func = gen_lambda(
            &engine,
            "exec_rust_lambda_function.wasm",
            1024 * 1024 * 2,
            get_default_ip(),
        )
        .await;

        let result = func.run("HELLO WORLD").await.unwrap();
        assert_eq!("HELLO WORLD### TEST ###", result);
    }

    #[tokio::test]
    async fn stop_infinite_loop_function() {
        let engine = gen_engine();
        let func = Arc::new(tokio::sync::RwLock::new(
            gen_lambda(
                &engine,
                "stop_infinite_loop.wasm",
                1024 * 1024 * 2,
                get_default_ip(),
            )
            .await,
        ));

        let handler = tokio::spawn({
            let func_ref = func.clone();
            async move {
                let lref = func_ref.read().await;
                lref.run("").await
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let _ = func.read().await.stop().await;
        let handler_stop = handler.await.unwrap();
        if let Err(e) = handler_stop {
            if let Some(func_err) = e.downcast_ref::<LambdaError>() {
                assert_eq!(*func_err, LambdaError::ForceStop);
                return;
            }
        }
        panic!();
    }

    #[tokio::test]
    async fn multiple_function_execution() {
        let engine = gen_engine();
        let func = Arc::new(tokio::sync::RwLock::new(
            gen_lambda(
                &engine,
                "multiple_function_exec.wasm",
                1024 * 1024 * 2,
                get_default_ip(),
            )
            .await,
        ));

        let handler_one = tokio::spawn({
            let func = func.clone();
            async move {
                let func = func.read().await;
                func.run("f,e,d,c,b,a").await
            }
        });

        let handler_two = tokio::spawn({
            let func = func.clone();
            async move {
                let func = func.read().await;
                func.run("e,d,c,b,a").await
            }
        });

        let (res_one, res_two) =
            tokio::spawn(async move { (handler_one.await, handler_two.await) })
                .await
                .unwrap();

        assert_eq!("[a,b,c,d,e,f]", res_one.unwrap().unwrap());
        assert_eq!("[a,b,c,d,e]", res_two.unwrap().unwrap());
    }

    #[tokio::test]
    async fn tcp_udp_bind_ip_test() {
        let engine = gen_engine();
        let func = gen_lambda(
            &engine,
            "tcp_udp_bind_to_not_allowed_ip.wasm",
            1024 * 1024 * 2,
            get_default_ip(),
        )
        .await;

        // NOTE: Allowed ip for tcp/udp
        assert_eq!(
            "### TCP ###",
            func.run("TCP,127.0.0.1:50400").await.unwrap()
        );
        assert_eq!(
            "### UDP ###",
            func.run("UDP,127.0.0.1:50400").await.unwrap()
        );

        // NOTE: Not allowed ip for tcp/udp
        if let Err(e) = func.run("TCP,192.168.2.2.3:50300").await {
            if let Some(func_err) = e.downcast_ref::<LambdaError>() {
                assert_eq!(LambdaError::FunctionExecError, *func_err);
            } else {
                panic!();
            }
        }

        if let Err(e) = func.run("UDP,192.168.2.2.3:50300").await {
            if let Some(func_err) = e.downcast_ref::<LambdaError>() {
                assert_eq!(LambdaError::FunctionExecError, *func_err);
            } else {
                panic!();
            }
        }
    }

    // NOTE: Utility functions

    fn get_default_ip() -> Ipv4Addr {
        Ipv4Addr::new(127, 0, 0, 1)
    }

    fn gen_engine() -> Engine {
        let mut config = Config::new();
        config
            .async_support(true)
            .epoch_interruption(true)
            .cranelift_opt_level(OptLevel::SpeedAndSize);
        Engine::new(&config).unwrap()
    }

    fn load_component(engine: &Engine, path: &Path) -> wasmtime::component::Component {
        Component::from_file(engine, path).expect("Wasm module not found")
    }

    async fn gen_lambda(
        engine: &Engine,
        wasm_file: &str,
        memory_size: usize,
        tap_ip: Ipv4Addr,
    ) -> Lambda {
        let wasm_function_path = PathBuf::from(&format!("{}/{}", WASM_RESOURCES, wasm_file));
        let component = Arc::new(load_component(engine, &wasm_function_path));
        Lambda::new(component.clone(), memory_size, tap_ip)
            .await
            .unwrap()
    }
}
