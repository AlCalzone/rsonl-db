use thiserror::Error;

#[derive(Error, Debug)]
pub enum JsonlDBError {
  #[error("The DB is already open")]
  AlreadyOpen,
  #[error("The DB is not open")]
  NotOpen,
  #[error("The DB must be stopped to close the DB files")]
  NotStopped,

  #[error("The value {0:?} is not a primitive")]
  NotPrimitive(serde_json::Value),

  #[error("Invalid options")]
  InvalidOptions {
    #[from] source: anyhow::Error
  }
}

impl From<JsonlDBError> for napi::Error {
    fn from(error: JsonlDBError) -> Self {
      napi::Error::from_reason(format!("{:?}", error))
    }
}
