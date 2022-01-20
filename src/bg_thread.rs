use std::sync::{Arc};
use tokio::{
  sync::{mpsc::Sender, Notify},
  task::{JoinError, JoinHandle},
};

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
  pub async fn stop_and_join(&mut self) -> Result<T, JoinError> {
    self.send_command(Command::Stop).await.unwrap();
    self.thread.as_mut().await
  }

  pub async fn send_command(&mut self, cmd: Command) -> Result<(), JoinError> {
    self.tx.send(cmd).await.unwrap();
    Ok(())
  }
}
