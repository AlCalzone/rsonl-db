use std::io::Error;
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tokio::{
  fs::File,
  io::{AsyncBufReadExt, BufReader},
};

#[derive(Clone)]
pub(crate) enum MapValue {
  Stringified(String),
  Raw(serde_json::Value),
}

impl From<MapValue> for serde_json::Value {
  fn from(val: MapValue) -> Self {
    match val {
      MapValue::Stringified(str) => serde_json::from_str(&str).unwrap(),
      MapValue::Raw(v) => v,
    }
  }
}

impl From<&MapValue> for serde_json::Value {
  fn from(val: &MapValue) -> Self {
    match val {
      MapValue::Stringified(str) => serde_json::from_str(str).unwrap(),
      MapValue::Raw(v) => v.clone(),
    }
  }
}

impl Into<String> for MapValue {
  fn into(self) -> String {
    match self {
      MapValue::Stringified(str) => str,
      MapValue::Raw(v) => serde_json::to_string(&v).unwrap(),
    }
  }
}

pub(crate) fn format_line(key: &str, val: impl Into<String>) -> String {
  format!(
    "{{\"k\":{},\"v\":{}}}\n",
    serde_json::to_string(key).unwrap(),
    val.into()
  )
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub(crate) enum Entry {
  Value { k: String, v: serde_json::Value },
  Delete { k: String },
}

pub(crate) async fn parse_entries(
  file: &mut File,
  ignore_read_errors: bool,
) -> Result<IndexMap<String, MapValue>, Error> {
  let mut entries = IndexMap::<String, MapValue>::new();

  let mut lines = BufReader::new(file).lines();
  while let Some(line) = lines.next_line().await? {
    let entry = serde_json::from_str::<Entry>(&line);
    match entry {
      Ok(Entry::Value { k, v }) => {
        entries.insert(k, MapValue::Raw(v));
      }
      Ok(Entry::Delete { k }) => {
        entries.remove(&k);
      }
      Err(e) => {
        if ignore_read_errors {
          // ignore read errors
        } else {
          return Err(e.into());
        }
      }
    }
  }

  Ok(entries)
}

pub(crate) type Backlog = Vec<String>;

pub(crate) struct Storage {
  pub entries: IndexMap<String, MapValue>,
  pub backlog: Backlog,
}

#[derive(Clone)]
pub(crate) struct SharedStorage(Arc<Mutex<Storage>>);

impl SharedStorage {
  pub fn new(s: Storage) -> Self {
    Self(Arc::new(Mutex::new(s)))
  }

  pub fn lock(
    &mut self,
  ) -> Result<std::sync::MutexGuard<Storage>, std::sync::PoisonError<std::sync::MutexGuard<Storage>>>
  {
    self.0.lock()
  }

  pub fn len(&mut self) -> usize {
    let storage = self.lock().unwrap();
    let entries = &storage.entries;
    entries.len()
  }

  pub fn insert(&mut self, key: String, value: MapValue, stringified: String) {
    let mut storage = self.lock().unwrap();
    storage.entries.insert(key, value);
    storage.backlog.push(stringified);
  }

  pub fn remove(&mut self, key: String) {
    let mut stringified = serde_json::to_string(&Entry::Delete { k: key.to_owned() }).unwrap();
    stringified.push_str("\n");

    let mut storage = self.lock().unwrap();
    storage.entries.remove(&key);
    storage.backlog.push(stringified);
  }

  pub fn clear(&mut self) {
    let mut storage = self.lock().unwrap();
    storage.entries.clear();
    storage.backlog.push("".to_owned());
  }

  pub fn drain_backlog(&mut self) -> Vec<String> {
    let mut storage = self.lock().unwrap();
    let backlog = &mut storage.backlog;
    backlog.splice(.., []).collect()
  }

  pub fn drain_backlog_if<F>(&mut self, predicate: F) -> Vec<String>
  where
    F: Fn(&Vec<String>) -> bool,
  {
    let mut storage = self.lock().unwrap();
    let backlog = &mut storage.backlog;
    if !predicate(&backlog) {
      return vec![];
    }
    backlog.splice(.., []).collect()
  }
}
