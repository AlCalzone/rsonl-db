use std::io::Error;

use indexmap::IndexMap;
use serde::{Serialize, Deserialize};
use tokio::{fs::File, io::{BufReader, AsyncBufReadExt}};

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub(crate) enum Entry {
  Value { k: String, v: serde_json::Value },
  Delete { k: String },
}

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

pub(crate) async fn parse_entries(file: &mut File, ignore_read_errors: bool) -> Result<IndexMap<String, MapValue>, Error> {
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
