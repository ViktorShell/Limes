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
}
