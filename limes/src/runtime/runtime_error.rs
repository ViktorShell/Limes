use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum RuntimeError {
    #[error("RuntimeError: Could not initialize the engine")]
    EngineInitError,
    #[error("RuntimeError: Could not initialize the Module due to byte code errors")]
    ComponentBuildError,
    #[error("RuntimeError: Module already registered")]
    ModuleAlreadyReg,
    #[error("RuntimeError: Lambda function failed to execute")]
    LambdaFailedExec,
    #[error("RuntimeError: This module is not registered")]
    ComponentNotFound,
    #[error("RuntimeError: The max allocation of functions was reached")]
    MaxFunctionDeplaymentReached,
    #[error("RuntimeError: Was not able to init the function due to `{0}`")]
    FunctionInitError(String),
    #[error("RuntimeError: This functions is already initialized")]
    FunctionAlreadyInitialized,
    #[error("RuntimeError: Function was not able to execute due to `{0}`")]
    FunctionExecError(String),
    #[error("RuntimeError: Function was not able to stop due to `{0}`")]
    FunctionStopError(String),
}
