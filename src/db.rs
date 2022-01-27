use std::io::Error;
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
use crate::js_values::{map_to_object, vec_to_array, JsValue};
use crate::lockfile::Lockfile;
use crate::persistence::persistence_thread;
use crate::storage::{parse_entries, DBEntry, Index, JournalEntry, SharedStorage, Storage};
use crate::util::{replace_dirname, safe_parent};

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
        compress_promise: None,
      },
    })
  }
}

impl RsonlDB<HalfClosed> {
  pub fn close(&mut self, env: napi::Env) -> Result<RsonlDB<Closed>, Error> {
    {
      // Unref all native objects
      let mut storage = self.state.storage.lock().unwrap();
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
  pub async fn close(&mut self) -> Result<RsonlDB<HalfClosed>, Error> {
    // Compress if that is desired
    if self.options.auto_compress.on_close {
      self.compress().await?;
    }

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

  pub fn set_native(&mut self, key: String, value: serde_json::Value) {
    self.state.index.add_value_checked(&key, &value);
    self.state.storage.insert(key, DBEntry::Native(value));
  }

  pub fn set_reference(
    &mut self,
    key: String,
    obj: Ref<()>,
    stringified: String,
    index_keys: Vec<String>,
  ) {
    self.state.index.add_many(&key, index_keys);
    self
      .state
      .storage
      .insert(key, DBEntry::Reference(stringified, obj));
  }

  pub fn delete(&mut self, key: String) -> bool {
    if !self.has(&key) {
      return false;
    };

    self.state.index.remove(&key);
    self.state.storage.remove(key);
    true
  }

  pub fn clear(&mut self) {
    self.state.index.clear();
    self.state.storage.clear();
  }

  pub fn has(&mut self, key: &String) -> bool {
    self.state.storage.lock().unwrap().entries.contains_key(key)
  }

  pub fn get(&mut self, env: napi::Env, key: &str) -> Option<JsValue> {
    let entries = &mut self.state.storage.lock().unwrap().entries;
    let mut entry = entries.entry(key.to_owned());

    get_or_convert_entry(env, &mut entry)
  }

  pub fn get_many(
    &mut self,
    env: napi::Env,
    start_key: &str,
    end_key: &str,
    obj_filter: Option<String>,
  ) -> Vec<JsValue> {
    let mut ret = Vec::new();

    let entries = &mut self.state.storage.lock().unwrap().entries;

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
      .filter(|key| start_key.lt(key.as_str()) && end_key.gt(key.as_str()))
      .map(|k| k.to_owned())
      .collect();

    for key in keys {
      let mut entry = entries.entry(key.to_owned());

      if let Some(v) = get_or_convert_entry(env, &mut entry) {
        ret.push(v);
      }
    }
    ret
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
    let mut file = OpenOptions::new()
      .create(true)
      .truncate(true)
      .write(true)
      .open(filename)
      .await?;

    let json: String = {
      let entries = &self.state.storage.lock().unwrap().entries;

      let map = Map::<String, Value>::from_iter(
        entries.iter().map(|(k, v)| (k.to_owned(), Value::from(v))),
      );
      if pretty {
        serde_json::to_string_pretty(&map)?
      } else {
        serde_json::to_string(&map)?
      }
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
      self.state.index.add_value_checked(&key, &value);
      storage.entries.insert(key.clone(), DBEntry::Native(value));
      storage.journal.push(JournalEntry::Set(key));
    }

    Ok(())
  }
}

fn get_or_convert_entry(env: napi::Env, entry: &mut Entry<String, DBEntry>) -> Option<JsValue> {
  match entry {
    Entry::Occupied(e) => match e.get_mut() {
      DBEntry::Reference(_, r) => {
        let obj: JsObject = env.get_reference_value(r).unwrap();
        Some(JsValue::Object(obj))
      }

      DBEntry::Native(val) if val.is_array() => {
        let vec = val.as_array().unwrap().to_owned();
        let stringified = serde_json::to_string(&vec).unwrap();

        let arr = vec_to_array(env, vec).unwrap();
        let reference = env.create_reference(&arr).unwrap();
        e.insert(DBEntry::Reference(stringified, reference));

        Some(JsValue::Object(arr))
      }

      DBEntry::Native(val) if val.is_object() => {
        let map = val.as_object().unwrap().to_owned();
        let stringified = serde_json::to_string(&map).unwrap();

        let obj = map_to_object(env, map).unwrap();
        let reference = env.create_reference(&obj).unwrap();
        e.insert(DBEntry::Reference(stringified, reference));

        Some(JsValue::Object(obj))
      }

      DBEntry::Native(val) => Some(JsValue::Primitive(val.clone())),
    },
    Entry::Vacant(_) => None,
  }
}
