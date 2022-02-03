use thiserror::Error;

pub type Result<T> = std::result::Result<T, JsonlDBError>;

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
  InvalidOptions { source: anyhow::Error },

  #[error("IO operation failed")]
  IoError(#[from] std::io::Error),

  #[error("{reason}")]
  SerializeError {
    #[source]
    source: serde_json::Error,
    reason: String,
  },

  #[error("{reason}")]
  AsyncError {
    #[source]
    source: anyhow::Error,
    reason: String,
  },

  #[error(transparent)]
  NapiError(#[from] napi::Error),

  #[error(transparent)]
  Other(#[from] anyhow::Error),
}

impl From<JsonlDBError> for napi::Error {
  fn from(error: JsonlDBError) -> Self {
    napi::Error::from_reason(format!("{:?}", error))
  }
}

impl JsonlDBError {
  pub fn io_error_from_reason(reason: String) -> Self {
    std::io::Error::new(std::io::ErrorKind::Other, reason).into()
  }

  pub fn serde_to_string_failed(e: serde_json::Error) -> Self {
    Self::SerializeError {
      reason: "Failed to serialize to JSON".to_owned(),
      source: e,
    }
  }

  pub fn other(reason: &str) -> Self {
    anyhow::anyhow!(reason.to_owned()).into()
  }
}
