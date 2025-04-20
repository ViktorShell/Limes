use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::pin::Pin;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::*;
use wasmtime_wasi::{IoView, SocketAddrUse, WasiCtx, WasiCtxBuilder, WasiView};

struct State {
    wasi_ctx: WasiCtx,
    resource_table: ResourceTable,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = Config::new();
    config
        .async_support(true)
        .wasm_component_model(true)
        .cranelift_opt_level(OptLevel::Speed);
    let engine = Engine::new(&config).unwrap();
    let mut linker: Linker<State> = Linker::new(&engine);
    let _ = wasmtime_wasi::add_to_linker_async(&mut linker);
    let ip_check_closure = gen_check_ip_closure(Ipv4Addr::new(127, 0, 0, 1));

    let wasi = WasiCtxBuilder::new()
        .inherit_stdio()
        .socket_addr_check(ip_check_closure)
        .build();

    let resource_table = ResourceTable::new();

    let state = State {
        wasi_ctx: wasi,
        resource_table,
    };

    let component = Component::from_file(
        &engine,
        "/home/viktor/Documents/git/tesi/src/limes/limes/resources/test_multi_function_single_init/tcp_udp_bind_to_not_allowed_ip.wasm",
    ).unwrap();
    let mut store = Store::new(&engine, state);
    let instance = linker
        .instantiate_async(&mut store, &component)
        .await
        .unwrap();

    // Interface and function extractor
    let interface_idx = instance
        .get_export(&mut store, None, "component:run/run")
        .expect("world not found");

    let func_idx = instance
        .get_export(&mut store, Some(&interface_idx), "run")
        .expect("run interface not found");

    let func = instance
        .get_typed_func::<(&str,), (String,)>(&mut store, func_idx)
        .expect("Function not found");

    // Finalmente il Test
    let (res,) = func
        .call_async(&mut store, ("TCP,127.0.0.1:50402",))
        .await
        .unwrap();
    println!("TCP good => {}", res);
    func.post_return_async(&mut store).await;

    let (res,) = func
        .call_async(&mut store, ("UDP,127.0.0.1:50402",))
        .await
        .unwrap();
    println!("UDP good => {}", res);
    func.post_return_async(&mut store).await;

    let err = func
        .call_async(&mut store, ("TCP,192.168.1.12:50402",))
        .await
        .unwrap_err();
    println!("TCP bad");
    // let val = func.post_return_async(&mut store).await;

    let err = func
        .call_async(&mut store, ("UDP,192.168.1.13:50402",))
        .await
        .unwrap_err();
    println!("UDP bad");
    // let val = func.post_return_async(&mut store).await;

    Ok(())
}

fn gen_check_ip_closure(
    tap_ip: Ipv4Addr,
) -> Box<
    dyn Fn(SocketAddr, SocketAddrUse) -> Pin<Box<dyn Future<Output = bool> + Send + Sync>>
        + Send
        + Sync
        + 'static,
> {
    Box::new(move |socket, socket_check| {
        Box::pin(async move {
            println!("CLOSURE => {}", tap_ip);
            match socket_check {
                SocketAddrUse::TcpBind | SocketAddrUse::UdpBind => match socket {
                    SocketAddr::V4(socket_v4) => socket_v4.ip().eq(&tap_ip),
                    SocketAddr::V6(_) => false,
                },
                _ => true,
            }
        })
    })
}
