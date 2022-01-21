use std::io::Error;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use serde_json::{Map, Value};
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Notify};

use crate::bg_thread::{Command, ThreadHandle};
use crate::db_options::DBOptions;
use crate::lockfile::Lockfile;
use crate::persistence::persistence_thread;
use crate::storage::{format_line, parse_entries, Entry, MapValue, SharedStorage, Storage};
use crate::util::{replace_dirname, safe_parent};

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

impl RsonlDB<Closed> {
  pub fn new(filename: String, options: DBOptions) -> Self {
    RsonlDB {
      filename,
      options,
      state: Closed,
    }
  }

  async fn try_recover_db_files(&self) -> Result<(), Error> {
    let filename = self.filename.to_owned();
    let dump_filename = format!("{}.dump", &filename);
    let backup_filename = format!("{}.bak", &filename);

    // During the compression, the following sequence of events happens:
    // 1. A .jsonl.dump file gets written with a compressed copy of the data
    // 2. Files get renamed: .jsonl -> .jsonl.bak, .jsonl.dump -> .jsonl
    // 3. .bak file gets removed
    // 4. Buffered data gets written to the .jsonl file

    // This means if the .jsonl file is absent or truncated, we should be able to pick either the .dump or the .bak file
    // and restore the .jsonl file from it
    let mut db_file_ok = false;
    if let Ok(meta) = fs::metadata(&filename).await {
      db_file_ok = meta.is_file() && meta.len() > 0;
    }

    // Prefer the DB file if it exists, remove the others in case they exist
    if db_file_ok {
      fs::remove_file(&backup_filename).await.ok();
      fs::remove_file(&dump_filename).await.ok();
      return Ok(());
    }

    // The backup file should have complete data - the dump file could be subject to an incomplete write
    let mut bak_file_ok = false;
    if let Ok(meta) = fs::metadata(&backup_filename).await {
      bak_file_ok = meta.is_file() && meta.len() > 0;
    }

    if bak_file_ok {
      // Overwrite the broken db file with it and delete the dump file
      fs::rename(&backup_filename, &filename).await?;
      fs::remove_file(&dump_filename).await.ok();
      return Ok(());
    }

    // Try the dump file as a last attempt
    let mut dump_file_ok = false;
    if let Ok(meta) = fs::metadata(&dump_filename).await {
      dump_file_ok = meta.is_file() && meta.len() > 0;
    }

    if dump_file_ok {
      // Overwrite the broken db file with it and delete the backup file
      fs::rename(&dump_filename, &filename).await?;
      fs::remove_file(&backup_filename).await.ok();
      return Ok(());
    }

    Ok(())
  }

  pub async fn open(&self) -> Result<RsonlDB<Opened>, Error> {
    // Make sure the DB dir exists
    let db_dir = safe_parent(&self.filename).unwrap();
    fs::create_dir_all(&db_dir).await?;

    // Try to acquire a lock on the DB
    let lockfile_directory = match self.options.lockfile_directory.as_str() {
      "." => &db_dir,
      dir => Path::new(dir),
    };
    fs::create_dir_all(&lockfile_directory).await?;
    let lockfile_name = replace_dirname(format!("{}.lock", &self.filename), lockfile_directory)?;
    let mut lock = Lockfile::new(lockfile_name, 10000);
    lock.lock()?;

    // Make sure that there are no remains of a previous broken compress attempt
    // and restore a DB backup if it exists.
    self.try_recover_db_files().await?;

    let mut file = OpenOptions::new()
      .create(true)
      .read(true)
      .write(true)
      .open(&self.filename)
      .await?;

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
      persistence_thread(&filename, file, shared_storage, lock, rx, &opts)
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

  pub async fn export_json(&mut self, filename: &str, pretty: bool) -> Result<(), Error> {
    let entries = { self.state.storage.lock().unwrap().entries.clone() };

    let mut file = OpenOptions::new()
      .create(true)
      .truncate(true)
      .write(true)
      .open(filename)
      .await?;

    let map =
      Map::<String, Value>::from_iter(entries.iter().map(|(k, v)| (k.to_owned(), Value::from(v))));
    let json = if pretty {
      serde_json::to_string_pretty(&map)?
    } else {
      serde_json::to_string(&map)?
    };

    file.write_all(json.as_bytes()).await?;

    Ok(())
  }

  pub async fn import_json_file(&mut self, filename: &str) -> Result<(), Error> {
    let buffer = {
      let mut buffer = Vec::new();
      let mut file = OpenOptions::new().read(true).open(filename).await?;
      file.read_to_end(&mut buffer).await?;
      buffer
    };

    let json: Map<String, Value> = serde_json::from_slice(&buffer).unwrap();
    self.import_json_map(json)?;
    Ok(())
  }

  pub fn import_json_string(&mut self, json: &str) -> Result<(), Error> {
    let json: Map<String, Value> = serde_json::from_str(&json).unwrap();
    self.import_json_map(json)?;
    Ok(())
  }

  fn import_json_map(&mut self, map: Map<String, Value>) -> Result<(), Error> {
    let mut storage = self.state.storage.lock().unwrap();
    for (key, value) in map.into_iter() {
      let stringified = serde_json::to_string(&Entry::Value {
        k: key.clone(),
        v: value.clone(),
      })
      .unwrap();

      storage.entries.insert(key, MapValue::Raw(value));
      storage.backlog.push(format!("{}\n", stringified));
    }

    Ok(())
  }
}
