use super::lambda_error::LambdaError;
use std::future::Future;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wasmtime::component::{Component, Instance, Linker, ResourceTable};
use wasmtime::*;
use wasmtime_wasi::{IoView, SocketAddrUse, WasiCtx, WasiCtxBuilder, WasiView};

#[derive(Clone)]
pub enum LambdaStatus {
    Running,
    Ready,
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

pub struct Lambda {
    store: Store<LambdaState>,
    instance: Instance,
    stop: Arc<AtomicBool>,
}

impl Lambda {
    pub async fn new(
        engine: Arc<Engine>,
        component: Arc<Component>,
        memory_size: usize, // Pagine da 64Kbyte, minimo 2Mb -> 1024 * 1024 * 2
        tap_ip: Ipv4Addr,
    ) -> Result<(Self, impl Fn() -> Result<(), LambdaError>), LambdaError> {
        let mut linker = Linker::new(&*engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)
            .map_err(|_| LambdaError::WasiAsyncLinkerError)?;

        // TCP/UDP listener over TAP
        // FIX: TAP DEALLOCATED AFTER FIRST CLOSURE CALL, FOR NOW USING ARC
        let tap_ip = tap_ip;
        let check_ip: Box<
            dyn Fn(SocketAddr, SocketAddrUse) -> Pin<Box<dyn Future<Output = bool> + Send + Sync>>
                + Send
                + Sync
                + 'static,
        > = Box::new(move |socket, socket_check| {
            let tap_ip = tap_ip;
            Box::pin(async move {
                match socket_check {
                    SocketAddrUse::TcpBind | SocketAddrUse::UdpBind => match socket {
                        SocketAddr::V4(socket_v4) => socket_v4.ip().eq(&tap_ip),
                        SocketAddr::V6(_) => false,
                    },
                    _ => true,
                }
            })
        });

        // Wasi & StoreLimits
        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .socket_addr_check(check_ip)
            .build();
        let resource = ResourceTable::new();

        if memory_size < 1024 * 1024 * 2 {
            return Err(LambdaError::NotEnoughtMemory);
        }
        let store_limits = StoreLimitsBuilder::new().memory_size(memory_size).build();

        // State usend in Store<T> T = LambdaState
        let state = LambdaState {
            wasi_ctx: wasi,
            resource_table: resource,
            limiter: store_limits,
        };
        // Init store with memory limits
        let mut store: Store<LambdaState> = Store::new(&engine, state);
        store.limiter(|state| &mut state.limiter);

        // Get the Component Instance
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .map_err(|_| LambdaError::InstanceBuilderError)?;

        // Interrupt mechanism
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        store.epoch_deadline_callback(move |_store| {
            if !stop_clone.load(Ordering::Relaxed) {
                return Ok(UpdateDeadline::Yield(1));
            }
            Err(LambdaError::ForceStop.into())
        });

        // Force stop closure
        let stop_ref = Arc::clone(&stop);
        let engine_ref = Arc::clone(&engine);
        let stop_closure = move || {
            if stop_ref.load(Ordering::Relaxed) {
                return Err(LambdaError::FunctionNotRunning);
            }
            stop_ref.store(true, Ordering::Relaxed);
            engine_ref.increment_epoch();
            Ok(())
        };

        Ok((
            Lambda {
                store,
                instance,
                stop,
            },
            stop_closure,
        ))
    }

    pub async fn run(&mut self, args: &str) -> Result<String, LambdaError> {
        let instance = &self.instance;
        let mut store = &mut self.store;

        let interface_idx = instance
            .get_export(&mut store, None, "component:run/run")
            .ok_or(LambdaError::FunctionInterfaceError)
            .map_err(|e| e)?;

        let func_idx = instance
            .get_export(&mut store, Some(&interface_idx), "run")
            .ok_or(LambdaError::FunctionInterfaceRetrievError)
            .map_err(|e| e)?;

        let func = instance
            .get_typed_func::<(&str,), (String,)>(&mut store, func_idx)
            .map_err(|_| LambdaError::FunctionRetrievError)?;

        let stop_copy = Arc::clone(&self.stop);
        let result = func
            .call_async(&mut store, (args,))
            .await
            // TODO: Add wasi error message instead of FunctionExecError
            .map_err(move |_| {
                if stop_copy.load(Ordering::Relaxed) {
                    LambdaError::ForceStop
                } else {
                    LambdaError::FunctionExecError
                }
            })?
            .0;

        Ok(result)
    }
}
