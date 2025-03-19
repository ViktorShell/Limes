use super::lambda::{LambdaError, LambdaStatus, RunnableLambdaFunc};
use std::rc::Rc;
use wasmtime::*;

// FIX: Change Rc to RwLock
// FIX: Remove engine & module, doesn't need to be inside the struct
#[allow(dead_code)]
pub struct LambdaRust {
    engine: Rc<Engine>,
    store: Store<StoreLimits>,
    module: Rc<Module>,
    instance: Instance,
}

impl RunnableLambdaFunc for LambdaRust {
    fn new(
        engine: Rc<Engine>,
        module: Rc<Module>,
        memory_size: usize, // Pagine da 64Kbyte, minimo 2Mb -> 1024 * 1024 * 2
    ) -> Result<Self, LambdaError> {
        let store_limits = StoreLimitsBuilder::new().memory_size(memory_size).build();
        let mut store: Store<StoreLimits> = Store::new(engine.as_ref(), store_limits);
        store.limiter(|limit| limit);
        let instance = match Instance::new(&mut store, module.as_ref(), &[]) {
            Ok(instance) => instance,
            Err(e) => return Err(LambdaError::InstanceBuilderError(e.to_string())),
        };

        Ok(LambdaRust {
            engine: Rc::clone(&engine),
            store,
            module,
            instance,
        })
    }

    fn run(&mut self, args: &str) -> Result<String, LambdaError> {
        let store = &mut self.store;
        let instance = self.instance;

        // Access memory method
        let wasm_mem = match instance.get_memory(&mut *store, "memory") {
            Some(mem) => mem,
            None => {
                return Err(LambdaError::MemoryFunctionError(
                    "Memory function not found in the module".to_string(),
                ))
            }
        };

        // Allocator for wasm inner memory
        let wasm_alloc = instance
            .get_typed_func::<i32, i32>(&mut *store, "wasm_alloc")
            .map_err(|_| {
                LambdaError::FunctionRetrievError(
                    "Function wasm_alloc not found in the rust module".to_string(),
                )
            })?;

        // Run function -> the main one
        let wasm_wrapper = instance
            .get_typed_func::<(i32, i32), i32>(&mut *store, "wrapper")
            .map_err(|_| {
                LambdaError::FunctionRetrievError(
                    "Function wrapper not found in the rust module".to_string(),
                )
            })?;

        // Allocating memory for the args in the runtime
        let args_len = args.len();
        let args_ptr = match wasm_alloc.call(&mut *store, args_len as i32) {
            Ok(ptr) => ptr as usize,
            Err(e) => return Err(LambdaError::FunctinExecError(e.to_string())),
        };

        // Mapping data on runtime
        let mem_slice = wasm_mem.data_mut(&mut *store);
        if args_ptr + args_len <= mem_slice.len() {
            mem_slice[args_ptr..args_ptr + args_len].copy_from_slice(args.as_bytes());
        }

        // Exec run function
        let res_ptr = match wasm_wrapper.call(&mut *store, (args_ptr as i32, args_len as i32)) {
            Ok(ptr) => ptr as usize,
            Err(e) => return Err(LambdaError::FunctinExecError(e.to_string())),
        };

        // Reading result
        let data_slice = &wasm_mem.data(&*store)[res_ptr..];

        // Wasm limitation
        let sub_slice = if let Some(pos) = data_slice.iter().position(|&x| x == b'\0') {
            &data_slice[0..pos]
        } else {
            data_slice
        };

        let result = String::from_utf8_lossy(sub_slice).to_string();

        // Return result
        Ok(result)
    }

    fn get_status(&self) -> LambdaStatus {
        todo!();
    }

    fn set_status(&mut self, _status: LambdaStatus) {
        todo!();
    }
}
