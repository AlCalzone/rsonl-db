use std::sync::Arc;
use tokio::{
  sync::{mpsc::Sender, Notify},
  task::JoinHandle,
};

use crate::error::JsonlDBError;

pub(crate) type Callback = Arc<Notify>;

#[derive(Debug)]
pub(crate) enum Command {
  Stop,
  Dump { filename: String, done: Callback },
  Compress { done: Option<Callback> },
}

pub(crate) struct ThreadHandle<T> {
  pub thread: Box<JoinHandle<T>>,
  pub tx: Sender<Command>,
}

impl<T> ThreadHandle<T> {
  pub async fn stop_and_join(&mut self) -> Result<T, JsonlDBError> {
    self.send_command(Command::Stop).await?;
    self.thread.as_mut().await.or_else(|e| {
      Err(JsonlDBError::AsyncError {
        reason: "Joining the background task failed".to_owned(),
        source: e.into(),
      })
    })
  }

  pub async fn send_command(&mut self, cmd: Command) -> Result<(), JsonlDBError> {
    self.tx.send(cmd).await.or_else(|e| {
      Err(JsonlDBError::AsyncError {
        reason: "Failed to send command to background task".to_owned(),
        source: e.into(),
      })
    })?;
    Ok(())
  }
}
