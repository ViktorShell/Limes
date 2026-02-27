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
use wasmtime::Store;
use wasmtime::StoreLimits;
use wasmtime::{
    component::{Component, Instance, Linker, ResourceTable, TypedFunc},
    StoreLimitsBuilder,
};
use wasmtime_wasi::{DirPerms, FilePerms};
use wasmtime_wasi::{IoView, SocketAddrUse, WasiCtx, WasiCtxBuilder, WasiView};

pub type UserId = String;
pub type ModuleId = String;
pub type FunctionId = String;

pub struct ModuleHandler(Component);

#[derive(PartialEq, PartialOrd)]
pub enum FunctionStatus {
    Ready,
    Running,
    Stopped,
}

pub struct FunctionHandler {
    lambda: Lambda,
    status: FunctionStatus,
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
        let wasi_ctx = self.get_wasictx(args);
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
        // WARNING: You have to use args passing and operations to return the output from the user

        Ok("k".to_string())
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
            .get_export(&mut *store, Some(&interface_idx), "_start")
            .ok_or(anyhow::anyhow!(
                "Didn't find the component:run/run -> _start"
            ))?;

        instance
            .get_typed_func::<(&str,), (String,)>(store, func_idx)
            .with_context(|| "Function Retriev Error")
    }

    fn init_interrupt_callback(&self, store: &mut Store<LambdaState>) {
        let stop = self.stop.clone();
        store.epoch_deadline_callback(move |_| {
            if stop.load(Ordering::Relaxed) {
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
            wasi_ctx: wasi_ctx,
            resource_table: resource,
            limiter: store_limits,
        };
        let mut store = Store::new(self.component.engine(), state);
        store.limiter(|data| &mut data.limiter);
        store
    }

    fn get_wasictx(&self, args: &str) -> WasiCtx {
        // Ip connection filter
        let tap_ip = self.tap_ip.clone();
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
            .args(&[&args])
            .socket_addr_check(f_socket_check)
            .build()
    }
}
