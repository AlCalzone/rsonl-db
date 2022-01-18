#![deny(clippy::all)]

use std::path::Path;

use napi::bindgen_prelude::*;
use napi_derive::napi;

#[macro_use]
extern crate derive_builder;

#[cfg(all(
  any(windows, unix),
  target_arch = "x86_64",
  not(target_env = "musl"),
  not(debug_assertions)
))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod db;
mod db_options;
mod jsonldb_options;

#[macro_use]
mod error;
use db::{Closed, Opened, RsonlDB};
use jsonldb_options::JsonlDBOptions;


enum DB {
  Closed(RsonlDB<Closed>),
  Opened(RsonlDB<Opened>),
}

impl DB {
  fn is_opened(&self) -> bool {
    match self {
      DB::Opened(_) => true,
      _ => false,
    }
  }

  fn as_opened_mut(&mut self) -> Option<&mut RsonlDB<Opened>> {
    match self {
      DB::Opened(x) => Some(x),
      _ => None,
    }
  }

  fn as_closed_mut(&mut self) -> Option<&mut RsonlDB<Closed>> {
    match self {
      DB::Closed(x) => Some(x),
      _ => None,
    }
  }
}

#[napi(js_name = "JsonlDB")]
pub struct JsonlDB {
  r: DB,
}

#[napi(js_name = "JsonlDB")]
impl JsonlDB {
  #[napi(constructor)]
  pub fn new(filename: String, options: Option<JsonlDBOptions>) -> Self {
    let path = Path::new(&filename);
    JsonlDB {
      r: DB::Closed(RsonlDB::new(path.to_owned(), options.into())),
    }
  }

  #[napi]
  pub async fn open(&mut self) -> Result<()> {
    let db = self.r.as_closed_mut().ok_or(jserr!("DB is already open"))?;
    let db = db.open().await?;
    self.r = DB::Opened(db);

    Ok(())
  }

  #[napi]
  pub async fn close(&mut self) -> Result<()> {
    let db = self.r.as_opened_mut().ok_or(jserr!("DB is not open"))?;
    let db = db.close().await?;
    self.r = DB::Closed(db);

    Ok(())
  }

  #[napi]
  pub fn is_open(&self) -> bool {
    self.r.is_opened()
  }

  #[napi]
  pub fn add(&mut self, key: String, value: serde_json::Value) -> Result<()> {
    let db = self.r.as_opened_mut().ok_or(jserr!("DB is not open"))?;
    db.add(key, value);

    Ok(())
  }

  #[napi]
  pub fn add_serialized(&mut self, key: String, value: String) -> Result<()> {
    let db = self.r.as_opened_mut().ok_or(jserr!("DB is not open"))?;
    db.add_serialized(key, value);

    Ok(())
  }

  #[napi]
  pub fn delete(&mut self, key: String) -> Result<()> {
    let db = self.r.as_opened_mut().ok_or(jserr!("DB is not open"))?;
    db.delete(key);

    Ok(())
  }

  #[napi]
  pub fn has(&mut self, key: String) -> Result<bool> {
    let db = self.r.as_opened_mut().ok_or(jserr!("DB is not open"))?;
    Ok(db.has(&key))
  }

  #[napi]
  pub fn get(&mut self, key: String) -> Result<Option<serde_json::Value>> {
    let db = self.r.as_opened_mut().ok_or(jserr!("DB is not open"))?;
    Ok(db.get(&key))
  }

  #[napi]
  pub fn clear(&mut self) -> Result<()> {
    let db = self.r.as_opened_mut().ok_or(jserr!("DB is not open"))?;
    db.clear();
    Ok(())
  }

  #[napi(getter)]
  pub fn size(&mut self) -> Result<u32> {
    let db = self.r.as_opened_mut().ok_or(jserr!("DB is not open"))?;
    Ok(db.size() as u32)
  }
}
