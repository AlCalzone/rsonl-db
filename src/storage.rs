use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::vec;

use crate::error::{JsonlDBError, Result};

use indexmap::IndexMap;
use napi::{Env, Ref};
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

impl TryFrom<&DBEntry> for serde_json::Value {
  type Error = JsonlDBError;

  fn try_from(value: &DBEntry) -> std::result::Result<Self, Self::Error> {
    match value {
      DBEntry::Reference(str, _) => {
        serde_json::from_str(str).map_err(|e| JsonlDBError::SerializeError {
          reason: format!("Could not convert stringified entry {str}"),
          source: e,
        })
      }
      DBEntry::Native(v) => Ok(v.clone()),
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

pub(crate) fn drop_safe(env: Env, entry: Option<DBEntry>) {
  if let Some(e) = entry {
    match e {
      DBEntry::Reference(_, mut r) => {
        // referenced JS objects MUST be unref'ed
        r.unref(env).ok();
        drop(r);
      }
      DBEntry::Native(v) => {
        drop(v);
      }
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
) -> Result<IndexMap<String, DBEntry>> {
  let mut entries = IndexMap::<String, DBEntry>::new();

  let mut lines = BufReader::new(file).lines();
  let mut line_no: u32 = 0;
  while let Some(line) = lines.next_line().await? {
    let entry = serde_json::from_str::<Entry>(&line);
    line_no += 1;
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
          return Err(JsonlDBError::SerializeError {
            reason: format!("Cannot open DB file: Invalid data in line {line_no}"),
            source: e,
          });
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

  pub fn lock(&mut self) -> MutexGuard<'_, Storage> {
    // If we cannot lock the mutex, crashing doesn't seem like the worst option.
    self
      .0
      .lock()
      .map_err(|_| JsonlDBError::other("Failed to acquire lock on storage"))
      .unwrap()
  }

  pub fn len(&mut self) -> usize {
    let storage = self.lock();
    let entries = &storage.entries;
    entries.len()
  }

  // pub fn journal_len(&mut self) -> usize {
  //   let storage = self.lock();
  //   storage.journal.len()
  // }

  pub fn insert(&mut self, key: String, value: DBEntry) -> Option<DBEntry> {
    let mut storage = self.lock();
    let old = storage.entries.insert(key.clone(), value);
    // Deduplicate while inserting, removing all previous pending writes for this key
    storage.journal.retain(|e| match e {
      JournalEntry::Set(k) if k == &key => false,
      JournalEntry::Delete(k) if k == &key => false,
      _ => true,
    });
    storage.journal.push(JournalEntry::Set(key));
    old
  }

  pub fn remove(&mut self, key: String) -> Option<DBEntry> {
    let mut storage = self.lock();
    let ret = storage.entries.remove(&key);
    // Deduplicate while inserting, removing all previous pending writes for this key
    storage.journal.retain(|e| match e {
      JournalEntry::Set(k) if k == &key => false,
      JournalEntry::Delete(k) if k == &key => false,
      _ => true,
    });
    storage.journal.push(JournalEntry::Delete(key));
    ret
  }

  pub fn clear(&mut self) -> Vec<DBEntry> {
    let mut storage = self.lock();
    let ret = storage.entries.drain(..).map(|(_, e)| e).collect();
    // All pending writes are obsolete, remove them from the journal
    storage.journal.clear();
    storage.journal.push(JournalEntry::Clear);
    ret
  }

  pub fn drain_journal(&mut self) -> Vec<String> {
    let mut storage = self.lock();

    let journal: Vec<JournalEntry> = storage.journal.splice(.., []).collect();

    journal
      .into_iter()
      .filter_map(|j| journal_entry_to_string(&storage.entries, &j))
      .collect()
  }

  pub fn drain_journal_if<F>(&mut self, predicate: F) -> Vec<String>
  where
    F: Fn(&Vec<JournalEntry>) -> bool,
  {
    let mut storage = self.lock();
    let journal = &mut storage.journal;
    if !predicate(&journal) {
      return vec![];
    }

    let journal: Vec<JournalEntry> = journal.splice(.., []).collect();
    journal
      .into_iter()
      .filter_map(|j| journal_entry_to_string(&storage.entries, &j))
      .collect()
  }

  pub fn clone_journal(&mut self) -> Vec<String> {
    let storage = self.lock();
    storage
      .journal
      .clone()
      .into_iter()
      .filter_map(|j| journal_entry_to_string(&storage.entries, &j))
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
    JournalEntry::Clear => Some("".to_string()),
  }
}
