use std::collections::VecDeque;
use std::io::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::mpsc::{self, Sender};
use tokio::task::JoinHandle;
use tokio::time;

use serde::{Deserialize, Serialize};

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
  // backlogs for writing
  write_backlog: Arc<Mutex<Vec<Entry>>>,
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

    // We keep two references to the write backlog, one for putting entries in, one for reading it in the BG thread
    let write_backlog = Arc::new(Mutex::new(Vec::<Entry>::new()));
    let write_backlog2 = write_backlog.clone();

    let (tx, mut rx) = mpsc::channel(32);
    let thread = tokio::spawn(async move {
      let mut writer = BufWriter::new(file);
      let idle_duration = Duration::from_millis(100);

      loop {
        let command = time::timeout(idle_duration, rx.recv()).await;

        // Grab all entries from the backlog
        let backlog: Vec<Entry> = {
          let mut write_backlog2 = write_backlog2.lock().unwrap();
          write_backlog2.splice(.., []).collect()
        };

        // And print them
        for entry in backlog.iter() {
          let str = serde_json::to_string(&entry).unwrap();
          writer.write(str.as_bytes()).await.unwrap();
          writer.write(b"\n").await.unwrap();
        }

        if let Ok(Some(Command::Stop)) = command {
          break;
        }
      }

      // If we're stopping, make sure everything gets written
      writer.flush().await.unwrap();
    });

    // Change the state to Opened
    Ok(RsonlDB {
      filename: self.filename.to_owned(),
      state: Opened {
        thread: Box::new(thread),
        tx,
        write_backlog,
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

  pub fn add(&mut self, key: String, value: serde_json::Value) {
    let entry = Entry {
      k: key.to_owned(),
      v: value,
    };

    let mut write_backlog = self.state.write_backlog.lock().unwrap();
    write_backlog.push(entry);
  }
}
