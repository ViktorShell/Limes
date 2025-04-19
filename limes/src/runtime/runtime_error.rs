use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum RuntimeError {
    #[error("bitch")]
    OkMaybe,
}
