use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum LambdaError {
    #[error("Wasm memory function error")]
    MemoryFunctionError,
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
}

//impl fmt::Display for LambdaError {
//    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//        match self {
//            LambdaError::MemoryFunctionError(err) => {
//                write!(f, "Wasm retriving memory error => {}", err)
//            }
//            LambdaError::FunctionRetrievError(err) => {
//                write!(f, "Wasm function retriev error => {}", err)
//            }
//            LambdaError::FunctinExecError(err) => {
//                write!(f, "Wasm function exec error => {}", err)
//            }
//            LambdaError::ModuleNotFound(err) => {
//                write!(f, "Wasm function builder error => {}", err)
//            }
//            LambdaError::InstanceBuilderError(err) => {
//                write!(f, "Wasm function builder error => {}", err)
//            }
//            LambdaError::ForceStop => {
//                write!(f, "Wasm module stopped")
//            }
//            LambdaError::EngineBuildError => {
//                write!(f, "Engine build error")
//            }
//        }
//    }
//}
//
//impl fmt::Debug for LambdaError {
//    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//        match self {
//            LambdaError::MemoryFunctionError(err) => {
//                write!(f, "Wasm retriving memory error => {}", err)
//            }
//            LambdaError::FunctionRetrievError(err) => {
//                write!(f, "Wasm function retriev error => {}", err)
//            }
//            LambdaError::FunctinExecError(err) => {
//                write!(f, "Wasm function exec error => {}", err)
//            }
//            LambdaError::ModuleNotFound(err) => {
//                write!(f, "Wasm function builder error => {}", err)
//            }
//            LambdaError::InstanceBuilderError(err) => {
//                write!(f, "Wasm function builder error => {}", err)
//            }
//            LambdaError::ForceStop => {
//                write!(f, "Wasm module stopped")
//            }
//            LambdaError::EngineBuildError => {
//                write!(f, "Engine build error")
//            }
//        }
//    }
//}
//
//impl Error for LambdaError {}
