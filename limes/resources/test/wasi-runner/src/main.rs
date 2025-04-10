use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::*;
use wasmtime_wasi::{IoView, WasiCtx, WasiCtxBuilder, WasiView};

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

    let wasi = WasiCtxBuilder::new().inherit_stdio().inherit_args().build();
    let state = State {
        wasi_ctx: wasi,
        resource_table: ResourceTable::new(),
    };

    let component = Component::from_file(
        &engine,
        "/home/viktor/Desktop/wasi-test/wasi-runner/run.wasm",
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
