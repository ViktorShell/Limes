use super::lambda_error::LambdaError;
use std::collections::HashMap;
use std::future::Future;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wasmtime::component::{Component, Instance, Linker, ResourceTable, TypedFunc};
use wasmtime::*;
use wasmtime_wasi::DirPerms;
use wasmtime_wasi::FilePerms;
use wasmtime_wasi::{IoView, SocketAddrUse, WasiCtx, WasiCtxBuilder, WasiView};

pub struct LambdaState {
    wasi_ctx: WasiCtx, // WARN: Doesn't implement Sync to prevent memory movemnts
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
    component: Arc<Component>,
    memory_size: usize,
    tap_ip: Ipv4Addr,
    stop: Arc<AtomicBool>,
    wasi_flags: WasiFlags,
}

pub struct WasiFlags {
    socket_addr_check: Option<()>,
    file_mapper: Option<HashMap<String, (String, DirPerms, FilePerms)>>,
}

impl WasiFlags {
    pub fn new(
        socket_addr_check: Option<()>,
        file_mapper: Option<HashMap<String, (String, DirPerms, FilePerms)>>,
    ) -> Self {
        Self {
            socket_addr_check,
            file_mapper,
        }
    }
}

impl Default for WasiFlags {
    fn default() -> Self {
        Self {
            socket_addr_check: Some(()),
            file_mapper: None,
        }
    }
}

impl Lambda {
    pub async fn new(
        component: Arc<Component>, // WARN: Cross-engine not supported, Engine and Component need to have the same key
        memory_size: usize,
        tap_ip: Ipv4Addr,
        wasi_flags: WasiFlags,
    ) -> Result<Self, LambdaError> {
        if memory_size < 1024 * 1024 * 2 {
            return Err(LambdaError::NotEnoughtMemory);
        }
        let stop = Arc::new(AtomicBool::new(false));
        Ok(Self {
            component,
            memory_size,
            tap_ip,
            stop,
            wasi_flags,
        })
    }

    pub async fn run(&self, args: &str) -> Result<String, LambdaError> {
        // Setup the Linker and Wasi support
        let engine = self.component.engine();
        let mut linker = Linker::new(engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)
            .map_err(|e| LambdaError::WasiAsyncLinkerError(e.to_string()))?;
        let wasi_ctx = self.build_wasi_ctx();
        let mut store = self.build_store(engine, wasi_ctx);

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

        // Reset the store even though it will be de-allocated.
        // I will remove it soon and change the way the function exec.
        // let _ = func.post_return_async(&mut store).await;
        Ok(result)
    }

    pub async fn stop(&self) -> Result<(), LambdaError> {
        let engine = self.component.engine();
        if self.stop.load(Ordering::Relaxed) {
            return Err(LambdaError::FunctionNotRunning);
        }
        self.stop.store(true, Ordering::Relaxed);
        engine.increment_epoch();
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

    fn build_wasi_ctx(&self) -> WasiCtx {
        let mut wasictx = WasiCtxBuilder::new();
        if let Some(_) = self.wasi_flags.socket_addr_check {
            let ip_checker = self.gen_check_ip_closure();
            wasictx.socket_addr_check(ip_checker);
        }
        if let Some(map) = &self.wasi_flags.file_mapper {
            for (host, guest) in map.iter() {
                let guest_path = guest.0.clone();
                let dir_perms = guest.1;
                let file_perms = guest.2;
                wasictx
                    .preopened_dir(host, guest_path, dir_perms, file_perms)
                    .expect("Could not map the files from host to the guest runtime");
            }
        }
        #[cfg(debug_assertions)]
        {
            wasictx.inherit_stdio();
        }
        wasictx.build()
    }

    fn build_store(&self, engine: &Engine, wasi: WasiCtx) -> Store<LambdaState> {
        let resource = ResourceTable::new();
        let store_limits = StoreLimitsBuilder::new()
            .memory_size(self.memory_size)
            .build();
        let state = LambdaState {
            wasi_ctx: wasi,
            resource_table: resource,
            limiter: store_limits,
        };
        let mut store = Store::new(engine, state);
        store.limiter(|data| &mut data.limiter);
        store
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
        let local_tap_ip = self.tap_ip.clone();
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
