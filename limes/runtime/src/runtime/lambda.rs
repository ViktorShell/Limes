use std::error::Error;
use std::fmt;
use std::rc::Rc;
use wasmtime::{Engine, Module};

pub enum LambdaStatus {
    Init,
    Run,
    Pause,
    Stop,
}

pub trait RunnableLambdaFunc {
    fn new(engine: Rc<Engine>, module: Rc<Module>, memory_size: usize) -> Result<Self, LambdaError>
    where
        Self: Sized;
    fn run(&mut self, args: &str) -> Result<String, LambdaError>;
    fn get_status(&self) -> LambdaStatus;
    fn set_status(&mut self, status: LambdaStatus);
}

pub enum LambdaError {
    MemoryFunctionError(String),
    FunctionRetrievError(String),
    FunctinExecError(String),
    ModuleNotFound(String),
    InstanceBuilderError(String),
}

impl fmt::Display for LambdaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LambdaError::MemoryFunctionError(err) => {
                write!(f, "Wasm retriving memory error => {}", err)
            }
            LambdaError::FunctionRetrievError(err) => {
                write!(f, "Wasm function retriev error => {}", err)
            }
            LambdaError::FunctinExecError(err) => {
                write!(f, "Wasm function exec error => {}", err)
            }
            LambdaError::ModuleNotFound(err) => {
                write!(f, "Wasm function builder error => {}", err)
            }
            LambdaError::InstanceBuilderError(err) => {
                write!(f, "Wasm function builder error => {}", err)
            }
        }
    }
}

impl fmt::Debug for LambdaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LambdaError::MemoryFunctionError(err) => {
                write!(f, "Wasm retriving memory error => {}", err)
            }
            LambdaError::FunctionRetrievError(err) => {
                write!(f, "Wasm function retriev error => {}", err)
            }
            LambdaError::FunctinExecError(err) => {
                write!(f, "Wasm function exec error => {}", err)
            }
            LambdaError::ModuleNotFound(err) => {
                write!(f, "Wasm function builder error => {}", err)
            }
            LambdaError::InstanceBuilderError(err) => {
                write!(f, "Wasm function builder error => {}", err)
            }
        }
    }
}

impl Error for LambdaError {}
