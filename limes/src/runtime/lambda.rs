use super::lambda_error::LambdaError;
use std::future::Future;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wasmtime::component::{Component, Instance, Linker, ResourceTable, TypedFunc};
use wasmtime::*;
use wasmtime_wasi::{IoView, SocketAddrUse, WasiCtx, WasiCtxBuilder, WasiView};

pub struct LambdaState {
    wasi_ctx: WasiCtx, // This motherfucker doesnà't implement the Sync trait
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

pub struct Lambda {
    engine: Arc<Engine>,
    component: Arc<Component>,
    memory_size: usize,
    tap_ip: Ipv4Addr,
    stop: Arc<AtomicBool>,
}

impl Lambda {
    pub async fn new(
        engine: Arc<Engine>,       // Cross-Engine key
        component: Arc<Component>, // Cross-Engine key
        memory_size: usize,
        tap_ip: Ipv4Addr,
    ) -> Result<Self, LambdaError> {
        if memory_size < 1024 * 1024 * 2 {
            return Err(LambdaError::NotEnoughtMemory);
        }
        let stop = Arc::new(AtomicBool::new(false));
        Ok(Self {
            engine,
            component,
            memory_size,
            tap_ip,
            stop,
        })
    }

    pub async fn run(&self, args: &str) -> Result<String, LambdaError> {
        // Setup the Linker and Wasi support
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)
            .map_err(|e| LambdaError::WasiAsyncLinkerError(e.to_string()))?;
        let mut store = self.store_with_wasi_support();

        // Store register epoch_deadline_callback
        let stop = self.stop.clone();
        store.epoch_deadline_callback(move |_| {
            if !stop.load(Ordering::Relaxed) {
                return Ok(UpdateDeadline::Yield(1));
            }
            Err(LambdaError::ForceStop.into())
        });

        // Get the function Instance from Component
        let instance = linker
            .instantiate_async(&mut store, &self.component)
            .await
            .map_err(|e| LambdaError::InstanceBuilderError(e.to_string()))?;

        // Get the run function
        let func = self.get_func_run(&instance, &mut store)?;

        // Exec the function
        let result = func
            .call_async(&mut store, (args,))
            .await
            .map_err(|_| match self.stop.load(Ordering::Relaxed) {
                true => LambdaError::ForceStop,
                false => LambdaError::FunctionExecError,
            })?
            .0;

        // Reset the store even though it will be deallocated
        // I will remove it soon and change the way function exec
        let _ = func.post_return_async(&mut store).await;
        Ok(result)
    }

    pub async fn stop(&self) -> Result<(), LambdaError> {
        if self.stop.load(Ordering::Relaxed) {
            return Err(LambdaError::FunctionNotRunning);
        }
        self.stop.store(true, Ordering::Relaxed);
        self.engine.increment_epoch();
        Ok(())
    }

    fn get_func_run(
        &self,
        instance: &Instance,
        store: &mut Store<LambdaState>,
    ) -> Result<TypedFunc<(&str,), (String,)>, LambdaError> {
        let interface_idx = instance
            .get_export(&mut *store, None, "component:run/run")
            .ok_or(LambdaError::FunctionInterfaceError)
            .map_err(|e| e)?;

        let func_idx = instance
            .get_export(&mut *store, Some(&interface_idx), "run")
            .ok_or(LambdaError::FunctionInterfaceRetrievError)
            .map_err(|e| e)?;

        Ok(instance
            .get_typed_func::<(&str,), (String,)>(&mut *store, func_idx)
            .map_err(|e| LambdaError::FunctionRetrievError(e.to_string()))?)
    }

    fn store_with_wasi_support(&self) -> Store<LambdaState> {
        let ip_checker = self.gen_check_ip_closure();
        let wasi = WasiCtxBuilder::new().socket_addr_check(ip_checker).build();
        let resource = ResourceTable::new();
        let store_limits = StoreLimitsBuilder::new()
            .memory_size(self.memory_size)
            .build();
        let state = LambdaState {
            wasi_ctx: wasi,
            resource_table: resource,
            limiter: store_limits,
        };
        Store::new(&self.engine, state)
    }

    // Closure for ip checks
    fn gen_check_ip_closure(
        &self,
    ) -> Box<
        dyn Fn(SocketAddr, SocketAddrUse) -> Pin<Box<dyn Future<Output = bool> + Send + Sync>>
            + Send
            + Sync
            + 'static,
    > {
        let local_tap_ip = self.tap_ip.clone(); // Fuck lifetimes for 4 bytes of data
        Box::new(move |socket, socket_check| {
            Box::pin(async move {
                match socket_check {
                    SocketAddrUse::TcpBind | SocketAddrUse::UdpBind => match socket {
                        SocketAddr::V4(socket_v4) => socket_v4.ip().eq(&local_tap_ip),
                        SocketAddr::V6(_) => false,
                    },
                    _ => true,
                }
            })
        })
    }
}
