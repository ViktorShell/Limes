use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum LambdaError {
    #[error("Wasm memory function error")]
    MemoryFunctionError,
    #[error("Wasm WIT interface not found")]
    FunctionInterfaceError,
    #[error("Wasm function interface not found")]
    FunctionInterfaceRetrievError,
    #[error("Wasm function not found due to `{0}`")]
    FunctionRetrievError(String),
    #[error("Wasm function exec error")]
    FunctionExecError,
    #[error("Wasm module not found")]
    ModuleNotFound,
    #[error("Wasm instance build error: `{0}`")]
    InstanceBuilderError(String),
    #[error("Wasm function was forced to stop")]
    ForceStop,
    #[error("Wasm could not build the Engine")]
    EngineBuildError,
    #[error("Arguments size > wasm module memory")]
    ArgsOutOfMemory,
    #[error("The module is not running")]
    FunctionNotRunning,
    #[error("Wasi was not able add async capabilities to the linker due to `{0}`")]
    WasiAsyncLinkerError(String),
    #[error("Allocate at least 2Mb of memory")]
    NotEnoughtMemory,
}
