use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Error, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Entry {
  k: String,
  v: serde_json::Value,
}

pub(crate) struct RsonlDB<S: DBState> {
  pub filename: PathBuf,
  pub state: S,
}

// Data that's only present in certain DB states
pub(crate) struct Closed;

pub(crate) struct Opened {
  writer: BufWriter<File>,
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
  pub fn new(filename: PathBuf) -> Self {
    RsonlDB {
      filename,
      state: Closed,
    }
  }

  pub fn open(&self) -> Result<RsonlDB<Opened>, Error> {
    let file = OpenOptions::new()
      .read(true)
      .append(true)
      .open(self.filename.to_owned())?;

    let writer = BufWriter::new(file);

    Ok(RsonlDB {
      filename: self.filename.to_owned(),
      state: Opened { writer },
    })
  }
}

impl RsonlDB<Opened> {
  pub fn close(&mut self) -> Result<RsonlDB<Closed>, Error> {
    self.state.writer.flush()?;
    Ok(RsonlDB {
      filename: self.filename.to_owned(),
      state: Closed,
    })
  }

  pub fn add(&mut self, key: String, value: serde_json::Value) -> std::io::Result<()> {
    let entry = Entry { k: key, v: value };

    serde_json::to_writer(&mut self.state.writer, &entry)?;
    self.state.writer.write(b"\n")?;
    Ok(())
  }
}
