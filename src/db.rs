use std::io::Error;
use std::str::FromStr;
use std::sync::Arc;

use tokio::fs::{File, OpenOptions};
use tokio::sync::{mpsc, Notify};

use crate::bg_thread::{Command, ThreadHandle};
use crate::db_options::DBOptions;
use crate::storage::{format_line, parse_entries, Entry, MapValue, SharedStorage, Storage};
use crate::persistence::persistence_thread;

pub(crate) struct RsonlDB<S: DBState> {
  pub filename: String,
  options: DBOptions,
  pub state: S,
}

// Data that's only present in certain DB states
pub(crate) struct Closed;

pub(crate) struct Opened {
  storage: SharedStorage,
  persistence_thread: ThreadHandle<()>,
  compress_promise: Option<Arc<Notify>>,
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

impl<S: DBState> RsonlDB<S> {
  async fn open_file(&self) -> Result<File, Error> {
    return Ok(
      OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&self.filename)
        .await?,
    );
  }
}

impl RsonlDB<Closed> {
  pub fn new(filename: String, options: DBOptions) -> Self {
    RsonlDB {
      filename,
      options,
      state: Closed,
    }
  }

  pub async fn open(&self) -> Result<RsonlDB<Opened>, Error> {
    let mut file = self.open_file().await?;

    // Read the entire file. This also puts the cursor at the end, so we can start writing
    let entries = parse_entries(&mut file, self.options.ignore_read_errors).await?;
    let backlog = Vec::<String>::new();
    let storage = SharedStorage::new(Storage { entries, backlog });

    let filename = self.filename.clone();
    let opts = self.options.clone();
    let shared_storage = storage.clone();

    // Start the write thread
    let (tx, rx) = mpsc::channel(32);
    let thread = tokio::spawn(async move {
      persistence_thread(&filename, file, shared_storage, rx, &opts)
        .await
        .unwrap();
    });

    // Now change the state to Opened
    Ok(RsonlDB {
      filename: self.filename.to_owned(),
      options: self.options.clone(),
      state: Opened {
        storage,
        persistence_thread: ThreadHandle {
          thread: Box::new(thread),
          tx,
        },
        compress_promise: None,
      },
    })
  }
}

impl RsonlDB<Opened> {
  pub async fn close(&mut self) -> Result<RsonlDB<Closed>, Error> {
    // Compress if that is desired
    if self.options.auto_compress.on_close {
      self.compress().await?;
    }

    // End the all threads and wait for them to end
    self.state.persistence_thread.stop_and_join().await?;

    // Free memory
    drop(&self.state);

    // Change DB state to closed
    Ok(RsonlDB {
      options: self.options.clone(),
      filename: self.filename.to_owned(),
      state: Closed,
    })
  }

  pub fn set(&mut self, key: String, value: serde_json::Value) {
    let stringified = serde_json::to_string(&Entry::Value {
      k: key.to_owned(),
      v: value.clone(),
    })
    .unwrap();

    self
      .state
      .storage
      .insert(key, MapValue::Raw(value), stringified);
  }

  pub fn set_stringified(&mut self, key: String, value: String) {
    let stringified = format_line(&key, &value);
    self
      .state
      .storage
      .insert(key, MapValue::Stringified(value), stringified);
  }

  pub fn delete(&mut self, key: String) -> bool {
    if !self.has(&key) {
      return false;
    };

    self.state.storage.remove(key);
    true
  }

  pub fn clear(&mut self) {
    self.state.storage.clear();
  }

  pub fn has(&mut self, key: &String) -> bool {
    self.state.storage.lock().unwrap().entries.contains_key(key)
  }

  pub fn get(&mut self, key: &String) -> Option<serde_json::Value> {
    let entries = &self.state.storage.lock().unwrap().entries;
    match entries.get(key) {
      Some(MapValue::Raw(val)) => Some(val.clone()),
      Some(MapValue::Stringified(str)) => Some(serde_json::Value::from_str(&str.clone()).unwrap()),
      None => None,
    }
  }

  pub fn size(&mut self) -> usize {
    self.state.storage.lock().unwrap().entries.len()
  }

  pub fn all_keys(&mut self) -> Vec<String> {
    let entries = &self.state.storage.lock().unwrap().entries;
    entries.keys().cloned().collect()
  }

  pub async fn dump(&mut self, filename: &str) -> Result<(), Error> {
    // Send command to the persistence thread
    let notify = Arc::new(Notify::new());
    self
      .state
      .persistence_thread
      .send_command(Command::Dump {
        filename: filename.to_owned(),
        done: notify.clone(),
      })
      .await
      .unwrap();

    // and wait until it is done
    notify.notified().await;

    Ok(())
  }

  pub async fn compress(&mut self) -> Result<(), Error> {
    // Don't compress twice in parallel and block all further calls
    if let Some(notify) = self.state.compress_promise.as_ref() {
      notify.clone().notified().await;
      return Ok(());
    } else {
      let notify = Arc::new(Notify::new());
      self.state.compress_promise = Some(notify.clone());

      // Send command to the persistence thread
      self
        .state
        .persistence_thread
        .send_command(Command::Compress {
          done: Some(notify.clone()),
        })
        .await
        .unwrap();

      // and wait until it is done
      notify.clone().notified().await;

      self.state.compress_promise = None;
    }

    Ok(())
  }
}
