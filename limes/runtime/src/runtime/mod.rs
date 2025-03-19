use phf::map::Map;
use uuid::Uuid;

pub mod lambda;
pub mod lambda_rust;

#[allow(dead_code)]
struct RuntimeHandler {
    vcpus: u8,
    memory: u32,
    modules: Map<Uuid, String>,
    ready_modules: Map<Uuid, String>,
    running_modules: Map<Uuid, String>,
}

#[allow(dead_code)]
impl RuntimeHandler {
    pub fn new() {
        todo!();
    }

    pub fn default() {
        todo!();
    }

    pub fn create() {
        todo!();
    }

    pub fn start() {
        todo!();
    }

    pub fn stop() {
        todo!();
    }

    pub fn is_running() {
        todo!();
    }

    pub fn is_ready() {
        todo!();
    }
}

#[cfg(test)]
mod test {
    use super::lambda::RunnableLambdaFunc;
    use crate::runtime::lambda_rust::LambdaRust;
    use std::path::PathBuf;
    use std::rc::Rc;
    use wasmtime::*;

    #[test]
    fn exec_rust_lambda_function() {
        let engine = Engine::default();

        // Path to wasm file
        let mut wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        wasm_path.pop();
        wasm_path.push("resources/rust_module/wrapper-rust/wrapper-rust.wasm");

        let module = match Module::from_file(&engine, wasm_path) {
            Ok(module) => module,
            Err(e) => panic!("{}", e),
        };

        let mut lambda_rust =
            LambdaRust::new(Rc::new(engine), Rc::new(module), 1024 * 1024 * 2).unwrap();

        let numbers = "1,2,3,4,5";
        let result = match lambda_rust.run(&numbers) {
            Ok(res) => res,
            Err(e) => panic!("{}", e),
        };
        assert_eq!("15", result);
    }
}
