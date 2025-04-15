use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::pin::Pin;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::*;
use wasmtime_wasi::{IoView, SocketAddrUse, WasiCtx, WasiCtxBuilder, WasiView};

pub struct State {
    pub wasi_ctx: WasiCtx,
    pub resource_table: ResourceTable,
}

impl IoView for State {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.resource_table
    }
}

impl WasiView for State {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }
}

fn main() -> Result<()> {
    let engine = Engine::default();
    let mut linker = Linker::new(&engine);
    let _ = wasmtime_wasi::add_to_linker_sync(&mut linker);

    let tap_ip = Ipv4Addr::new(127, 0, 0, 1);
    let check_socket: Box<
        dyn Fn(SocketAddr, SocketAddrUse) -> Pin<Box<dyn Future<Output = bool> + Send + Sync>>
            + Send
            + Sync
            + 'static,
    > = Box::new(move |socket, socket_check| {
        let tap_ip = tap_ip; // catturazione nel move async
        Box::pin(async move {
            match socket_check {
                SocketAddrUse::TcpBind | SocketAddrUse::UdpBind => match socket {
                    SocketAddr::V4(socket_v4) => {
                        println!("SOCKET IP: {}", socket_v4.ip().to_string());
                        socket_v4.ip().eq(&tap_ip)
                    }
                    SocketAddr::V6(_) => false,
                },
                _ => true,
            }
        })
    });

    let wasi = WasiCtxBuilder::new()
        .inherit_stdio()
        .socket_addr_check(check_socket)
        .build();
    let state = State {
        wasi_ctx: wasi,
        resource_table: ResourceTable::new(),
    };

    let component = Component::from_file(
        &engine,
        "/home/viktor/Documents/git/tesi/src/limes/limes/resources/test/wasi-runner/run_wasip2.wasm",
    )?;
    let mut store = Store::new(&engine, state);
    let instance = linker.instantiate(&mut store, &component)?;

    // Good
    let interface_idx = instance
        .get_export(&mut store, None, "component:run/run")
        .expect("Componente non trovato");

    let parent_export_idx = Some(&interface_idx);
    let func_idx = instance
        .get_export(&mut store, parent_export_idx, "run")
        .expect("Funzione del run");
    let func = instance
        .get_func(&mut store, func_idx)
        .expect("Unreachable since we've got func_idx");

    let typed = func.typed::<(&str,), (String,)>(&store)?;
    let (res,) = typed.call(&mut store, ("Hello ",))?;

    // Sintassi strana
    println!("Function call => {}", res);
    Ok(())
}
