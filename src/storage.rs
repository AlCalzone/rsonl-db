use std::collections::{HashMap, HashSet};
use std::io::Error;
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use napi::Ref;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
  fs::File,
  io::{AsyncBufReadExt, BufReader},
};

pub(crate) enum DBEntry {
  Reference(String, Ref<()>),
  Native(serde_json::Value),
}

#[derive(Clone)]
pub(crate) enum JournalEntry {
  Set(String),
  Delete(String),
  Clear,
}

impl From<&DBEntry> for serde_json::Value {
  fn from(val: &DBEntry) -> Self {
    match val {
      DBEntry::Reference(str, _) => serde_json::from_str(str).unwrap(),
      DBEntry::Native(v) => v.clone(),
    }
  }
}

impl Into<String> for DBEntry {
  fn into(self) -> String {
    match self {
      DBEntry::Reference(str, _) => str,
      DBEntry::Native(v) => serde_json::to_string(&v).unwrap(),
    }
  }
}

impl Into<String> for &DBEntry {
  fn into(self) -> String {
    match self {
      DBEntry::Reference(str, _) => str.to_owned(),
      DBEntry::Native(v) => serde_json::to_string(v).unwrap(),
    }
  }
}

pub(crate) fn format_line(key: &str, val: impl Into<String>) -> String {
  format!(
    "{{\"k\":{},\"v\":{}}}",
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
) -> Result<IndexMap<String, DBEntry>, Error> {
  let mut entries = IndexMap::<String, DBEntry>::new();

  let mut lines = BufReader::new(file).lines();
  while let Some(line) = lines.next_line().await? {
    let entry = serde_json::from_str::<Entry>(&line);
    match entry {
      Ok(Entry::Value { k, v }) => {
        entries.insert(k, DBEntry::Native(v));
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

pub(crate) type Journal = Vec<JournalEntry>;

pub(crate) struct Index {
  paths: Vec<String>,
  // (Map: "path=value" => (object keys[]))
  map: HashMap<String, HashSet<String>>,
}

impl Index {
  pub fn new(paths: Vec<String>) -> Self {
    Self {
      map: HashMap::new(),
      paths,
    }
  }

  pub fn add_entries_checked(&mut self, entries: &IndexMap<String, DBEntry>) {
    let paths = { self.paths.clone() };
    for (key, val) in entries {
      for path in &paths {
        if let DBEntry::Native(val) = val {
          // ... create a new index entry
          if let Some(index_val) = val.pointer(path).map_or(None, |v| v.as_str()) {
            let index_key = format!("{}={}", path, &index_val);
            self.add_one(&index_key, &key);
          }
        }
      }
    }
  }

  pub fn add_value_checked(&mut self, key: &str, val: &serde_json::Value) {
    let paths = { self.paths.clone() };
    for path in paths {
      if let Some(index_val) = val.pointer(&path).map_or(None, |v| v.as_str()) {
        let index_key = format!("{}={}", &path, &index_val);
        self.add_one(&index_key, &key);
      }
    }
  }

  pub fn add_one(&mut self, index_key: &str, key: &str) {
    let value_set = self
      .map
      .entry(index_key.to_owned())
      .or_insert_with(|| HashSet::new());
    value_set.insert(key.to_owned());
  }

  pub fn add_many(&mut self, key: &str, index_keys: Vec<String>) {
    for index_key in index_keys {
      self.add_one(&index_key, &key);
    }
  }

  // pub fn len(&self) -> usize {
  //   self.map.len()
  // }

  pub fn clear(&mut self) {
    self.map.clear();
  }

  pub fn remove(&mut self, key: &str) {
    for keys in self.map.values_mut() {
      keys.remove(key);
    }
  }

  pub fn get_keys(&self, index_key: &str) -> Option<Vec<String>> {
    match self.map.get(index_key) {
      Some(keys) => {
        let keys = keys.iter().cloned().collect();
        Some(keys)
      }
      None => None,
    }
  }
}

pub(crate) struct Storage {
  pub entries: IndexMap<String, DBEntry>,
  pub journal: Journal,
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

  pub fn insert(&mut self, key: String, value: DBEntry) {
    let mut storage = self.lock().unwrap();
    storage.entries.insert(key.clone(), value);
    storage.journal.push(JournalEntry::Set(key));
  }

  pub fn remove(&mut self, key: String) {
    let mut storage = self.lock().unwrap();
    storage.entries.remove(&key);
    storage.journal.push(JournalEntry::Delete(key));
  }

  pub fn clear(&mut self) {
    let mut storage = self.lock().unwrap();
    storage.entries.clear();
    storage.journal.push(JournalEntry::Clear);
  }

  pub fn drain_journal(&mut self) -> Vec<String> {
    let mut storage = self.lock().unwrap();

    let journal: Vec<JournalEntry> = storage.journal.splice(.., []).collect();
    journal
      .iter()
      .filter_map(|j| journal_entry_to_string(&storage.entries, j))
      .collect()
  }

  pub fn drain_journal_if<F>(&mut self, predicate: F) -> Vec<String>
  where
    F: Fn(&Vec<JournalEntry>) -> bool,
  {
    let mut storage = self.lock().unwrap();
    let journal = &mut storage.journal;
    if !predicate(&journal) {
      return vec![];
    }

    let journal: Vec<JournalEntry> = journal.splice(.., []).collect();
    journal
      .iter()
      .filter_map(|j| journal_entry_to_string(&storage.entries, j))
      .collect()
  }

  pub fn clone_journal(&mut self) -> Vec<String> {
    let storage = self.lock().unwrap();
    // let journal = &mut storage.journal;
    storage
      .journal
      .clone()
      .iter()
      .filter_map(|j| journal_entry_to_string(&storage.entries, j))
      .collect()
  }
}

fn journal_entry_to_string(
  entries: &IndexMap<String, DBEntry>,
  j: &JournalEntry,
) -> Option<String> {
  match j {
    JournalEntry::Set(key) => match entries.get(key) {
      Some(DBEntry::Native(v)) => Some(json!({ "k": key, "v": v }).to_string()),
      Some(DBEntry::Reference(str, _)) => Some(format!(
        "{{\"k\":{},\"v\":{}}}",
        serde_json::to_string(key).unwrap(),
        str
      )),
      // Skip entries that no longer exist
      None => None,
    },
    JournalEntry::Delete(key) => Some(json!({ "k": key }).to_string()),
    JournalEntry::Clear => Some("".to_owned()),
  }
}
