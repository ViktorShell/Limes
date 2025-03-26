use phf::map::Map;
use uuid::Uuid;
pub mod lambda_error;
pub mod lambda_rust;

#[allow(dead_code)]
struct Runtime {
    vcpus: u8,
    memory: u32,
    modules: Map<Uuid, String>,
    ready_modules: Map<Uuid, String>,
    running_modules: Map<Uuid, String>,
}

#[allow(dead_code)]
impl Runtime {
    pub fn new() {
        todo!();
    }

    pub fn register() {
        todo!();
    }

    pub fn remove() {
        todo!();
    }

    pub async fn start() {
        todo!();
    }

    pub async fn stop() {
        todo!();
    }

    pub async fn is_running() {
        todo!();
    }

    pub async fn is_ready() {
        todo!();
    }
}

impl Default for LimesRuntime {
    fn default() -> Self {
        todo!();
    }
}

#[cfg(test)]
mod test {
    use crate::runtime::lambda_error::LambdaError;
    use crate::runtime::lambda_rust::LambdaRust;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio;
    use wasmtime::*;

    #[tokio::test]
    async fn exec_rust_lambda_function() {
        // Engine
        let engine = Engine::new(Config::new().async_support(true).epoch_interruption(true));
        if let Err(e) = engine {
            panic!("Could not build the engine: {}", e)
        }
        let engine = Arc::new(engine.unwrap());

        // Path to wasm file
        let mut wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        wasm_path.pop();
        wasm_path.push("resources/rust_module/wrapper-rust/wrapper-rust.wasm");

        let module = Module::from_file(&*engine, wasm_path);
        if let Err(e) = module {
            panic!("Could not compile the module: {}", e);
        }
        let module = Arc::new(module.unwrap());

        let (mut func_exec, _func_stop) =
            LambdaRust::new(Arc::clone(&engine), Arc::clone(&module), 1024 * 1024 * 10)
                .await
                .unwrap();

        let numbers = "1,2,3,4,5";
        let result = func_exec.run(&numbers).await.unwrap();

        assert_eq!("15", result);
    }

    #[tokio::test]
    async fn stop_infinite_loop_function() {
        // Engine
        let engine = Engine::new(Config::new().async_support(true).epoch_interruption(true));
        if let Err(e) = engine {
            panic!("Could not build the engine: {}", e)
        }
        let engine = Arc::new(engine.unwrap());

        // Path to wasm file
        let mut wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        wasm_path.pop();
        wasm_path.push("resources/rust_module/infinite-loop-rust/infinite-loop-rust.wasm");

        let module = Module::from_file(&*engine, wasm_path);
        if let Err(e) = module {
            panic!("Could not compile the module: {}", e);
        }
        let module = Arc::new(module.unwrap());

        let (mut func_exec, func_stop) =
            LambdaRust::new(Arc::clone(&engine), Arc::clone(&module), 1024 * 1024 * 10)
                .await
                .unwrap();

        let numbers = "1,2,3,4,5";
        let handler = tokio::spawn(async move { func_exec.run(numbers).await });
        let fs_res = func_stop().unwrap();
        let _res = handler.await;

        assert_eq!((), fs_res);
    }

    #[tokio::test]
    async fn multiple_function_exec_one_interrupt() {
        let mut engines: Vec<Arc<Engine>> = Vec::new();
        for _ in 0..2 {
            let engine = Engine::new(Config::new().async_support(true).epoch_interruption(true));
            if let Err(e) = engine {
                panic!("Could not build the engine: {}", e);
            }
            engines.push(Arc::new(engine.unwrap()));
        }

        let crate_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut infinite_path = PathBuf::new();
        infinite_path.push(&crate_path);
        infinite_path.pop();
        infinite_path.push("resources/rust_module/infinite-loop-rust/infinite-loop-rust.wasm");

        let mut adder_path = PathBuf::new();
        adder_path.push(&crate_path);
        adder_path.pop();
        adder_path.push("resources/rust_module/wrapper-rust/wrapper-rust.wasm");

        let engine_inf = Arc::clone(&engines.get(0).unwrap());
        let inf_module = Module::from_file(&*engine_inf, infinite_path).unwrap();

        let engine_adder = Arc::clone(&engines.get(1).unwrap());
        let add_module = Module::from_file(&*engine_adder, adder_path).unwrap();

        let inf_module = Arc::new(inf_module);
        let add_module = Arc::new(add_module);

        let (mut inf_exec, inf_stop) = LambdaRust::new(
            Arc::clone(&engines.get(0).unwrap()),
            Arc::clone(&inf_module),
            1024 * 1024 * 10,
        )
        .await
        .unwrap();

        let (mut add_a_exec, _) = LambdaRust::new(
            Arc::clone(&engines.get(1).unwrap()),
            Arc::clone(&add_module),
            1024 * 1024 * 10,
        )
        .await
        .unwrap();

        let (mut add_b_exec, _) = LambdaRust::new(
            Arc::clone(&engines.get(1).unwrap()),
            Arc::clone(&add_module),
            1024 * 1024 * 10,
        )
        .await
        .unwrap();

        let handler_inf = tokio::spawn(async move { inf_exec.run("INFINITE").await });
        let handler_add_a = tokio::spawn(async move { add_a_exec.run("1,2,3,4,5").await });
        let handler_add_b = tokio::spawn(async move { add_b_exec.run("1,1,1,1,1").await });

        let _ = tokio::time::sleep(Duration::from_secs(1));
        let _inf_interrupt = inf_stop().unwrap();
        let (inf, add_a, add_b) = tokio::join!(handler_inf, handler_add_a, handler_add_b);

        let inf_res = if let Err(LambdaError::ForceStop) = inf.unwrap() {
            LambdaError::ForceStop
        } else {
            panic!("Infinite loop was not forced to stop");
        };

        let add_a_res = add_a.unwrap().unwrap();
        let add_b_res = add_b.unwrap().unwrap();

        assert_eq!(
            (LambdaError::ForceStop, "15", "5"),
            (inf_res, add_a_res.as_str(), add_b_res.as_str())
        );
    }
}
