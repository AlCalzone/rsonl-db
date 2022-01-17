#![deny(clippy::all)]

use std::path::Path;

use napi::bindgen_prelude::*;
use napi_derive::napi;

#[cfg(all(
  any(windows, unix),
  target_arch = "x86_64",
  not(target_env = "musl"),
  not(debug_assertions)
))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod db;
#[macro_use]
mod error;
use db::{Closed, Opened, RsonlDB};

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

  fn as_opened(&self) -> Option<&RsonlDB<Opened>> {
    match self {
      DB::Opened(x) => Some(x),
      _ => None,
    }
  }

  fn as_opened_mut(&mut self) -> Option<&mut RsonlDB<Opened>> {
    match self {
      DB::Opened(x) => Some(x),
      _ => None,
    }
  }

  fn as_closed(&self) -> Option<&RsonlDB<Closed>> {
    match self {
      DB::Closed(x) => Some(x),
      _ => None,
    }
  }

  fn as_closed_mut(&mut self) -> Option<&mut RsonlDB<Closed>> {
    match self {
      DB::Closed(x) => Some(x),
      _ => None,
    }
  }

  // fn into_opened(self) -> Option<RsonlDB<Opened>> {
  //   match self {
  //     DB::Opened(x) => Some(x),
  //     _ => None,
  //   }
  // }

  // fn into_closed(self) -> Option<RsonlDB<Closed>> {
  //   match self {
  //     DB::Closed(x) => Some(x),
  //     _ => None,
  //   }
  // }
}

#[napi]
pub struct JsonlDB {
  r: DB,
}

#[napi]
impl JsonlDB {
  #[napi(constructor)]
  pub fn new(filename: String) -> Self {
    let path = Path::new(&filename);
    JsonlDB {
      r: DB::Closed(RsonlDB::new(path.to_owned())),
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
}

#[napi]
pub fn serialize_test(str: serde_json::Value) -> String {
  str.to_string()
}
