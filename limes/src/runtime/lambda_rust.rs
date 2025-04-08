use super::lambda_error::LambdaError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wasmtime::component::{Component, Instance, Linker, ResourceTable};
use wasmtime::*;
use wasmtime_wasi::{IoView, WasiCtx, WasiCtxBuilder, WasiView};

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

pub struct LambdaRust {
    store: Store<LambdaState>,
    instance: Instance,
    stop: Arc<AtomicBool>,
}

impl LambdaRust {
    // pub async fn new(
    //     engine: Arc<Engine>,
    //     module: Arc<Module>,
    //     memory_size: usize, // Pagine da 64Kbyte, minimo 2Mb -> 1024 * 1024 * 2
    // ) -> Result<(Self, impl Fn() -> Result<(), LambdaError>), LambdaError> {
    //     // Store with memory size
    //     let store_limits = StoreLimitsBuilder::new().memory_size(memory_size).build();
    //     let mut store: Store<StoreLimits> = Store::new(&engine, store_limits);
    //     store.limiter(|limit| limit);
    //
    //     // Epoch, a way to yield the current running code & block it if needed
    //     let terminate = Arc::new(AtomicBool::new(false));
    //     let terminate_clone = Arc::clone(&terminate);
    //     store.epoch_deadline_callback(move |_store| {
    //         if !terminate_clone.load(Ordering::Relaxed) {
    //             Ok(UpdateDeadline::Yield(1))
    //         } else {
    //             return Err(LambdaError::ForceStop.into());
    //         }
    //     });
    //
    //     let instance = Instance::new_async(&mut store, &module, &[])
    //         .await
    //         .map_err(|_| LambdaError::InstanceBuilderError)?;
    //
    //     let terminate_ref = Arc::clone(&terminate);
    //     let engine_ref = Arc::clone(&engine);
    //     let stop_function = move || {
    //         if terminate_ref.load(Ordering::Relaxed) {
    //             return Err(LambdaError::FunctionNotRunning);
    //         }
    //         terminate_ref.store(true, Ordering::Relaxed);
    //         engine_ref.increment_epoch();
    //         Ok(())
    //     };
    //
    //     Ok((
    //         LambdaRust {
    //             store,
    //             instance,
    //             terminate,
    //         },
    //         stop_function,
    //     ))
    // }

    pub async fn new(
        engine: Arc<Engine>,
        component: Arc<Module>,
        memory_size: usize, // Pagine da 64Kbyte, minimo 2Mb -> 1024 * 1024 * 2
    ) -> Result<(Self, impl Fn() -> Result<(), LambdaError>), LambdaError> {
        // Init engine, linker and async component for wasi support
        let mut linker = wasmtime::Linker::new(&engine);
        // wasmtime_wasi::add_to_linker_async(&mut linker)
        //     .map_err(LambdaError::WasiAsyncLinkerError)?;

        match wasmtime_wasi::add_to_linker_async(&mut linker) {
            Err(_) => return Err(LambdaError::WasiAsyncLinkerError),
            _ => (),
        };

        // Wasi & StoreLimits
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();
        let resource = ResourceTable::new();
        let store_limits = StoreLimitsBuilder::new().memory_size(memory_size).build();

        // State usend in Store<T> T = LambdaState
        let mut state = LambdaState {
            wasi_ctx: wasi,
            resource_table: resource,
            limiter: store_limits,
        };

        // Init store with memory limits
        let mut store: Store<LambdaState> = Store::new(&engine, state);
        store.limiter(|state| &mut state.limiter);

        // Interrupt mechanism
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        store.epoch_deadline_callback(move |_store| {
            if !stop_clone.load(Ordering::Relaxed) {
                Ok(UpdateDeadline::Yield(1))
            } else {
                return Err(LambdaError::ForceStop.into());
            }
        });

        // Get the Component Instance
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .map_err(|_| LambdaError::InstanceBuilderError)?;

        // Force stop closure
        let stop_ref = Arc::clone(&stop);
        let engine_ref = Arc::clone(&engine);
        let stop_closure = move || {
            if stop_ref.load(Ordering::Relaxed) {
                return Err(LambdaError::FunctionNotRunning);
            }
            stop_ref.store(false, Ordering::Relaxed);
            engine_ref.increment_epoch();
            Ok(())
        };

        Ok((
            LambdaRust {
                store,
                instance,
                stop,
            },
            stop_closure,
        ))
    }

    // // togliere il riferimento e restituire una stringa owned
    // pub async fn run(&mut self, args: &str) -> Result<String, LambdaError> {
    //     let store = &mut self.store;
    //     let instance = self.instance;
    //
    //     // Access memory method
    //     let wasm_mem = instance
    //         .get_memory(&mut *store, "memory")
    //         .ok_or(LambdaError::MemoryFunctionError)?;
    //
    //     // Allocator for wasm inner memory
    //     let wasm_alloc = instance
    //         .get_typed_func::<i32, i32>(&mut *store, "wasm_alloc")
    //         .map_err(|_| LambdaError::FunctionRetrievError)?;
    //
    //     // Run function -> the main one
    //     let wasm_wrapper = instance
    //         .get_typed_func::<(i32, i32), i32>(&mut *store, "wrapper")
    //         .map_err(|_| LambdaError::FunctionRetrievError)?;
    //
    //     // Allocating memory for the args in the runtime
    //     let args_ptr = match wasm_alloc.call_async(&mut *store, args.len() as i32).await {
    //         Ok(ptr) => ptr as usize,
    //         Err(_) => {
    //             return {
    //                 if self.terminate.load(Ordering::Relaxed) {
    //                     Err(LambdaError::ForceStop)
    //                 } else {
    //                     Err(LambdaError::FunctionExecError)
    //                 }
    //             }
    //         }
    //     };
    //
    //     // Mapping data on runtime
    //     let mem_slice = wasm_mem.data_mut(&mut *store);
    //     if args_ptr + args.len() <= mem_slice.len() {
    //         mem_slice[args_ptr..args_ptr + args.len()].copy_from_slice(args.as_bytes());
    //     } else {
    //         return Err(LambdaError::ArgsOutOfMemory);
    //     }
    //
    //     // Exec run function
    //     let res_ptr = match wasm_wrapper
    //         .call_async(&mut *store, (args_ptr as i32, args.len() as i32))
    //         .await
    //     {
    //         Ok(ptr) => ptr as usize,
    //         Err(_) => {
    //             return {
    //                 if self.terminate.load(Ordering::Relaxed) {
    //                     Err(LambdaError::ForceStop)
    //                 } else {
    //                     Err(LambdaError::FunctionExecError)
    //                 }
    //             }
    //         }
    //     };
    //
    //     // Reading result
    //     let data_slice = &wasm_mem.data(&*store)[res_ptr..];
    //
    //     // Wasm limitation
    //     let sub_slice = if let Some(pos) = data_slice.iter().position(|&x| x == b'\0') {
    //         &data_slice[0..pos]
    //     } else {
    //         data_slice
    //     };
    //
    //     let result = String::from_utf8_lossy(sub_slice).to_string();
    //
    //     // Return result
    //     Ok(result)
    // }

    pub async fn run(&mut self, args: &str) -> Result<String, LambdaError> {
        let store = &mut self.store;
        let instance = self.instance;

        // Access memory method
        let wasm_mem = instance
            .get_memory(&mut *store, "memory")
            .ok_or(LambdaError::MemoryFunctionError)?;
    }
}
