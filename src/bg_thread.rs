use std::sync::{Arc, Mutex};
use tokio::{
  sync::mpsc::Sender,
  task::{JoinError, JoinHandle},
};

pub(crate) type Backlog = Arc<Mutex<Vec<String>>>;
pub(crate) type BGThread<T> = Option<Box<JoinHandle<T>>>;

#[derive(Debug, PartialEq)]
pub(crate) enum Command {
  Stop,
}

pub(crate) struct ThreadHandle<T> {
  pub thread: BGThread<T>,
  pub backlog: Backlog,
  pub tx: Option<Sender<Command>>,
}

impl<T> ThreadHandle<T> {
  pub async fn stop_and_join(&mut self) -> Result<Option<T>, JoinError> {
    if let Some(thread) = self.thread.as_mut() {
      if let Some(tx) = &self.tx {
        tx.send(Command::Stop).await.unwrap();
      }
      return thread.await.map(|r| Some(r));
    }
    Ok(None)
  }

  pub async fn join(&mut self) -> Result<Option<T>, JoinError> {
    if let Some(thread) = self.thread.as_mut() {
      return thread.await.map(|r| Some(r));
    }
    Ok(None)
  }

  pub fn push_backlog(&mut self, line: &str) {
    let mut backlog = self.backlog.lock().unwrap();
    if line == "" {
      backlog.clear()
    } else {
      backlog.push(line.to_owned());
    }
  }
}
