use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum LambdaError {
    #[error("Wasm memory function error")]
    MemoryFunctionError,
    #[error("Wasm WIT interface not found")]
    FunctionInterfaceError,
    #[error("Wasm function interface not found")]
    FunctionInterfaceRetrievError,
    #[error("Wasm function not found")]
    FunctionRetrievError,
    #[error("Wasm function exec error")]
    FunctionExecError,
    #[error("Wasm module not found")]
    ModuleNotFound,
    #[error("Wasm instance build error")]
    InstanceBuilderError,
    #[error("Wasm memory function error")]
    ForceStop,
    #[error("Wasm could not build the Engine")]
    EngineBuildError,
    #[error("Arguments size > wasm module memory")]
    ArgsOutOfMemory,
    #[error("The module is not running")]
    FunctionNotRunning,
    #[error("Wasi was not able add async capabilities to the linker")]
    WasiAsyncLinkerError,
    #[error("Allocate at least 2Mb of memory")]
    NotEnoughtMemory,
}
