use std::io::Error;
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Sender};
use tokio::task::JoinHandle;

#[derive(Serialize, Deserialize, Debug)]
struct Entry {
  k: String,
  v: serde_json::Value,
}

pub(crate) struct RsonlDB<S: DBState> {
  pub filename: PathBuf,
  pub state: S,
}

// Data that's only present in certain DB states
pub(crate) struct Closed;

pub(crate) struct Opened {
  // background thread
  thread: Box<JoinHandle<()>>,
  tx: Sender<Command>,
}

// Turn Opened/Closed into DB states
pub(crate) trait DBState {
  fn is_open(&self) -> bool;
}
impl DBState for Closed {
  fn is_open(&self) -> bool {
    false
  }
}
impl DBState for Opened {
  fn is_open(&self) -> bool {
    true
  }
}

#[derive(Debug)]
enum Command {
  Write(Entry),
  Stop,
}

impl RsonlDB<Closed> {
  pub fn new(filename: PathBuf) -> Self {
    RsonlDB {
      filename,
      state: Closed,
    }
  }

  pub async fn open(&self) -> Result<RsonlDB<Opened>, Error> {
    let file = OpenOptions::new()
      .read(true)
      .append(true)
      .open(self.filename.to_owned())
      .await?;

    let mut writer = BufWriter::new(file);

    let (tx, mut rx) = mpsc::channel(4096);
    let thread = tokio::spawn(async move {
      while let Some(message) = rx.recv().await {
        match message {
          Command::Write(entry) => {
            let str = serde_json::to_string(&entry).unwrap();
            writer.write(str.as_bytes()).await.unwrap();
            writer.write(b"\n").await.unwrap();
          }
          Command::Stop => {
            // Make sure everything gets written
            writer.flush().await.unwrap();
            return;
          }
        }
      }
    });

    Ok(RsonlDB {
      filename: self.filename.to_owned(),
      state: Opened {
        thread: Box::new(thread),
        tx,
      },
    })
  }
}

impl RsonlDB<Opened> {
  pub async fn close(&mut self) -> Result<RsonlDB<Closed>, Error> {
    // End the write thread and wait for it to end
    self.state.tx.send(Command::Stop).await.unwrap();
    self.state.thread.as_mut().await?;

    // Change DB state to closed
    Ok(RsonlDB {
      filename: self.filename.to_owned(),
      state: Closed,
    })
  }

  pub fn add_blocking(&mut self, key: String, value: serde_json::Value) -> std::io::Result<()> {
    let entry = Entry {
      k: key.to_owned(),
      v: value,
    };

    self.state.tx.blocking_send(Command::Write(entry)).unwrap();

    Ok(())
  }

  pub async fn add(&mut self, key: String, value: serde_json::Value) -> std::io::Result<()> {
    let entry = Entry {
      k: key.to_owned(),
      v: value,
    };

    self.state.tx.send(Command::Write(entry)).await.unwrap();

    Ok(())
  }

}
