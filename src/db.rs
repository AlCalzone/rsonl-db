use std::path::Path;
use std::sync::Arc;

use indexmap::map::Entry;
use napi::{JsObject, Ref};
use serde_json::{Map, Value};
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Notify};

use crate::bg_thread::{Command, ThreadHandle};
use crate::db_options::DBOptions;
use crate::error::{JsonlDBError, Result};
use crate::js_values::{map_to_object, vec_to_array, JsValue};
use crate::lockfile::Lockfile;
use crate::persistence::persistence_thread;
use crate::storage::{
  drop_safe, parse_entries, DBEntry, Index, JournalEntry, SharedStorage, Storage,
};
use crate::util::{parent_dir, replace_dirname};

pub(crate) struct RsonlDB<S: DBState> {
  pub filename: String,
  options: DBOptions,
  pub state: S,
}

// Data that's only present in certain DB states
pub(crate) struct Closed;

pub(crate) struct HalfClosed {
  storage: SharedStorage,
}

pub(crate) struct Opened {
  storage: SharedStorage,
  index: Index,
  persistence_thread: ThreadHandle<()>,
  compress_promise: Option<Arc<Notify>>,
  is_closing: bool
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
impl DBState for HalfClosed {
  fn is_open(&self) -> bool {
    false
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

  async fn try_recover_db_files(&self) -> Result<()> {
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

  pub async fn open(&self) -> Result<RsonlDB<Opened>> {
    // Make sure the DB dir exists
    let db_dir = parent_dir(&self.filename)?;
    fs::create_dir_all(&db_dir).await?;

    // Try to acquire a lock on the DB
    let lockfile_directory = match self.options.lockfile_directory.as_str() {
      "." => &db_dir,
      dir => Path::new(dir),
    };
    fs::create_dir_all(&lockfile_directory).await?;
    let lockfile_name = replace_dirname(format!("{}.lock", &self.filename), lockfile_directory)
      .ok_or_else(|| {
        JsonlDBError::io_error_from_reason(format!(
          "Could not determine lockfile name for \"{}\"",
          &self.filename
        ))
      })?;
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
    let journal = Vec::<JournalEntry>::new();
    let mut index = Index::new(self.options.index_paths.clone());
    index.add_entries_checked(&entries);

    let storage = SharedStorage::new(Storage { entries, journal });

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
        index,
        persistence_thread: ThreadHandle {
          thread: Box::new(thread),
          tx,
        },
        is_closing: false,
        compress_promise: None,
      },
    })
  }
}

impl RsonlDB<HalfClosed> {
  pub fn close(&mut self, env: napi::Env) -> Result<RsonlDB<Closed>> {
    {
      // Unref all native objects
      let mut storage = self.state.storage.lock();
      for entry in storage.entries.iter_mut() {
        if let DBEntry::Reference(_, r) = entry.1 {
          r.unref(env).ok();
        }
      }
    }

    // Free memory
    drop(&self.state);

    Ok(RsonlDB {
      options: self.options.clone(),
      filename: self.filename.to_owned(),
      state: Closed,
    })
  }
}

impl RsonlDB<Opened> {
  pub async fn close(&mut self) -> Result<RsonlDB<HalfClosed>> {
    // Compress if that is desired
    if self.options.auto_compress.on_close {
      self.compress().await?;
    }

    self.state.is_closing = true;

    // End the all threads and wait for them to end
    self.state.persistence_thread.stop_and_join().await?;

    // Change DB state to half-closed
    // Freeing memory has to happen on the Node.js thread
    Ok(RsonlDB {
      options: self.options.clone(),
      filename: self.filename.to_owned(),
      state: HalfClosed {
        storage: self.state.storage.to_owned(),
      },
    })
  }

  pub fn set_native(&mut self, env: napi::Env, key: String, value: serde_json::Value) {
    self.state.index.add_value_checked(&key, &value);
    let old = self.state.storage.insert(key, DBEntry::Native(value));
    drop_safe(env, old);
  }

  pub fn set_reference(
    &mut self,
    env: napi::Env,
    key: String,
    obj: Ref<()>,
    stringified: String,
    index_keys: Vec<String>,
  ) {
    self.state.index.add_many(&key, index_keys);
    let old = self
      .state
      .storage
      .insert(key, DBEntry::Reference(stringified, obj));
    drop_safe(env, old);
  }

  pub fn delete(&mut self, env: napi::Env, key: String) -> bool {
    if !self.has(&key) {
      return false;
    };

    self.state.index.remove(&key);
    let old = self.state.storage.remove(key);
    drop_safe(env, old);
    true
  }

  pub fn clear(&mut self, env: napi::Env) {
    self.state.index.clear();
    let old = self.state.storage.clear();

    for e in old {
      drop_safe(env, Some(e));
    }
  }

  pub fn has(&mut self, key: &String) -> bool {
    self.state.storage.lock().entries.contains_key(key)
  }

  pub fn get(&mut self, env: napi::Env, key: &str) -> Result<Option<JsValue>> {
    let entries = &mut self.state.storage.lock().entries;
    let mut entry = entries.entry(key.to_owned());

    get_or_convert_entry(env, &mut entry)
  }

  pub fn get_many(
    &mut self,
    env: napi::Env,
    start_key: &str,
    end_key: &str,
    obj_filter: Option<String>,
  ) -> Result<Vec<JsValue>> {
    let mut ret = Vec::new();

    let entries = &mut self.state.storage.lock().entries;

    let mut keys: Vec<String> = { entries.keys().cloned().into_iter().collect() };

    // If a filter is given, check if we have index entries that match it
    if let Some(obj_filter) = obj_filter {
      if let Some(index_keys) = self.state.index.get_keys(&obj_filter) {
        keys = index_keys;
      }
    }

    // Limit the results to the start_key...end_key range
    keys = keys
      .iter()
      .filter(|key| key.as_str().ge(start_key) && key.as_str().le(end_key))
      .map(|k| k.to_owned())
      .collect();

    for key in keys {
      let mut entry = entries.entry(key.to_owned());

      if let Some(v) = get_or_convert_entry(env, &mut entry)? {
        ret.push(v);
      }
    }
    Ok(ret)
  }

  pub fn size(&mut self) -> usize {
    self.state.storage.lock().entries.len()
  }

  pub fn all_keys(&mut self) -> Vec<String> {
    let entries = &self.state.storage.lock().entries;
    entries.keys().cloned().collect()
  }

  pub async fn dump(&mut self, filename: &str) -> Result<()> {
    // Don't do anything while the DB is being closed
    if self.state.is_closing {
      return Ok(());
    }

    // Send command to the persistence thread
    let notify = Arc::new(Notify::new());
    self
      .state
      .persistence_thread
      .send_command(Command::Dump {
        filename: filename.to_owned(),
        done: notify.clone(),
      })
      .await?;

    // and wait until it is done
    notify.notified().await;

    Ok(())
  }

  pub async fn compress(&mut self) -> Result<()> {
    // Don't do anything while the DB is being closed
    if self.state.is_closing {
      return Ok(());
    }

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
        .await?;

      // and wait until it is done
      notify.clone().notified().await;

      self.state.compress_promise = None;
    }

    Ok(())
  }

  pub async fn export_json(&mut self, filename: &str, pretty: bool) -> Result<()> {
    let mut file = OpenOptions::new()
      .create(true)
      .truncate(true)
      .write(true)
      .open(filename)
      .await?;

    let json: String = {
      let entries = &self.state.storage.lock().entries;

      let normalized_entries: Vec<(String, Value)> = entries
        .iter()
        .map(|(k, v)| match Value::try_from(v) {
          Ok(v) => Ok((k.to_owned(), v)),
          Err(e) => Err(e),
        })
        .collect::<Result<_>>()?;

      let map = Map::<String, Value>::from_iter(normalized_entries.into_iter());
      if pretty {
        serde_json::to_string_pretty(&map).map_err(|e| JsonlDBError::serde_to_string_failed(e))?
      } else {
        serde_json::to_string(&map).map_err(|e| JsonlDBError::serde_to_string_failed(e))?
      }
    };

    file.write_all(json.as_bytes()).await?;

    Ok(())
  }

  pub async fn import_json_file(&mut self, filename: &str) -> Result<()> {
    let buffer = {
      let mut buffer = Vec::new();
      let mut file = OpenOptions::new().read(true).open(filename).await?;
      file.read_to_end(&mut buffer).await?;
      buffer
    };

    let json: Map<String, Value> =
      serde_json::from_slice(&buffer).map_err(|e| JsonlDBError::SerializeError {
        reason: "Could not import JSON file".to_owned(),
        source: e,
      })?;
    self.import_json_map(json)?;
    Ok(())
  }

  pub fn import_json_string(&mut self, json: &str) -> Result<()> {
    let json: Map<String, Value> =
      serde_json::from_str(&json).map_err(|e| JsonlDBError::SerializeError {
        reason: "Could not import JSON string".to_owned(),
        source: e,
      })?;
    self.import_json_map(json)?;
    Ok(())
  }

  fn import_json_map(&mut self, map: Map<String, Value>) -> Result<()> {
    let mut storage = self.state.storage.lock();
    for (key, value) in map.into_iter() {
      self.state.index.add_value_checked(&key, &value);
      storage.entries.insert(key.clone(), DBEntry::Native(value));
      storage.journal.push(JournalEntry::Set(key));
    }

    Ok(())
  }
}

fn get_or_convert_entry(
  env: napi::Env,
  entry: &mut Entry<String, DBEntry>,
) -> Result<Option<JsValue>> {
  let result = match entry {
    Entry::Occupied(e) => match e.get_mut() {
      DBEntry::Reference(_, r) => {
        let obj: JsObject = env.get_reference_value(r)?;
        Some(JsValue::Object(obj))
      }

      DBEntry::Native(val) if val.is_array() => {
        let vec = val.as_array().unwrap().to_owned();
        let stringified =
          serde_json::to_string(&vec).map_err(|e| JsonlDBError::serde_to_string_failed(e))?;

        let arr = vec_to_array(env, vec)?;
        let reference = env.create_reference(&arr)?;
        e.insert(DBEntry::Reference(stringified, reference));

        Some(JsValue::Object(arr))
      }

      DBEntry::Native(val) if val.is_object() => {
        let map = val.as_object().unwrap().to_owned();
        let stringified =
          serde_json::to_string(&map).map_err(|e| JsonlDBError::serde_to_string_failed(e))?;

        let obj = map_to_object(env, map)?;
        let reference = env.create_reference(&obj)?;
        e.insert(DBEntry::Reference(stringified, reference));

        Some(JsValue::Object(obj))
      }

      DBEntry::Native(val) => Some(JsValue::Primitive(val.clone())),
    },
    Entry::Vacant(_) => None,
  };
  Ok(result)
}
