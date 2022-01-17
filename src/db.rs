use std::collections::HashMap;
use std::io::{Error, SeekFrom};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter, AsyncSeekExt};
use tokio::sync::mpsc::{self, Sender};
use tokio::task::JoinHandle;
use tokio::time;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Entry {
  k: String,
  v: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct DeleteEntry {
  k: String,
}

enum MapValue {
  Serialized(String),
  Raw(serde_json::Value),
}

pub(crate) struct RsonlDB<S: DBState> {
  pub filename: PathBuf,
  pub state: S,
}

// Data that's only present in certain DB states
pub(crate) struct Closed;

pub(crate) struct Opened {
  entries: HashMap<String, MapValue>,
  // background thread
  thread: Box<JoinHandle<()>>,
  tx: Sender<Command>,
  // backlogs for writing
  write_backlog: Arc<Mutex<Vec<String>>>,
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
      .share_mode(3)
      .open(self.filename.to_owned())
      .await?;

    // We keep two references to the write backlog, one for putting entries in, one for reading it in the BG thread
    let write_backlog = Arc::new(Mutex::new(Vec::<String>::new()));
    let write_backlog2 = write_backlog.clone();

    let (tx, mut rx) = mpsc::channel(32);
    let thread = tokio::spawn(async move {
      let mut writer = BufWriter::new(file);
      let idle_duration = Duration::from_millis(100);

      loop {
        let command = time::timeout(idle_duration, rx.recv()).await;

        // Grab all entries from the backlog
        let backlog: Vec<String> = {
          let mut write_backlog2 = write_backlog2.lock().unwrap();
          write_backlog2.splice(.., []).collect()
        };

        // And print them
        for str in backlog.iter() {
          if str == "" {
            // Truncate the file
            writer.get_ref().set_len(0).await.unwrap();
            writer.seek(SeekFrom::Start(0)).await.unwrap();
          } else {
            writer.write(str.as_bytes()).await.unwrap();
            writer.write(b"\n").await.unwrap();

          }
        }

        if let Ok(Some(Command::Stop)) = command {
          break;
        }
      }

      // If we're stopping, make sure everything gets written
      writer.flush().await.unwrap();
    });

    let entries = HashMap::<String, MapValue>::with_capacity(4096);

    // TODO: read from file

    // Change the state to Opened
    Ok(RsonlDB {
      filename: self.filename.to_owned(),
      state: Opened {
        entries,
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
    let str = serde_json::to_string(&Entry {
      k: key.to_owned(),
      v: value.clone(),
    })
    .unwrap();

    self.state.entries.insert(key, MapValue::Raw(value));

    let mut write_backlog = self.state.write_backlog.lock().unwrap();
    write_backlog.push(str);
  }

  pub fn add_serialized(&mut self, key: String, value: String) {
    let str = format!(
      "{{\"k\":{},\"v\":{}}}",
      serde_json::to_string(&key).unwrap(),
      value
    );

    self
      .state
      .entries
      .insert(key, MapValue::Serialized(value.clone()));

    let mut write_backlog = self.state.write_backlog.lock().unwrap();
    write_backlog.push(str);
  }

  pub fn delete(&mut self, key: String) {
    if !self.has(&key) {
      return;
    };

    let str = serde_json::to_string(&DeleteEntry { k: key.to_owned() }).unwrap();

    self.state.entries.remove(&key);

    {
      let mut write_backlog = self.state.write_backlog.lock().unwrap();
      write_backlog.push(str);
    }
  }

  pub fn clear(&mut self) {
    self.state.entries.clear();

    // Clear anything on the write backlog, it is obsolete now
    let mut write_backlog = self.state.write_backlog.lock().unwrap();
    write_backlog.clear();
    write_backlog.push("".to_owned());
  }

  pub fn has(&self, key: &String) -> bool {
    self.state.entries.contains_key(key)
  }

  pub fn get(&self, key: &String) -> Option<serde_json::Value> {
    match self.state.entries.get(key) {
      Some(MapValue::Raw(val)) => Some(val.clone()),
      Some(MapValue::Serialized(str)) => Some(serde_json::Value::from_str(&str.clone()).unwrap()),
      None => None,
    }
  }
}
