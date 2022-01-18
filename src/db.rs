use std::collections::BTreeMap;
use std::io::{Error, SeekFrom};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::mpsc::{self, Sender};
use tokio::task::JoinHandle;
use tokio::time::{self, Instant};

use serde::{Deserialize, Serialize};

use crate::db_options::DBOptions;

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum Entry {
  Value { k: String, v: serde_json::Value },
  Delete { k: String },
}

#[derive(Clone)]
pub enum MapValue {
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

pub(crate) struct RsonlDB<S: DBState> {
  pub filename: PathBuf,
  options: DBOptions,
  pub state: S,
}

// Data that's only present in certain DB states
pub(crate) struct Closed;

pub(crate) struct Opened {
  entries: BTreeMap<String, MapValue>,
  // background thread
  thread: Box<JoinHandle<()>>,
  tx: Sender<Command>,
  // backlogs for writing
  write_backlog: Arc<Mutex<Vec<String>>>,
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

#[derive(Debug)]
enum Command {
  Stop,
}

impl RsonlDB<Closed> {
  pub fn new(filename: PathBuf, options: DBOptions) -> Self {
    RsonlDB {
      filename,
      options,
      state: Closed,
    }
  }

  async fn parse_entries(&self, file: &mut File) -> Result<BTreeMap<String, MapValue>, Error> {
    let mut entries = BTreeMap::<String, MapValue>::new();

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
          if self.options.ignore_read_errors {
            // ignore read errors
          } else {
            return Err(e.into());
          }
        }
      }
    }

    Ok(entries)
  }

  async fn check_lf(&self, file: &mut File) -> Result<bool, Error> {
    if file.metadata().await?.len() > 0 {
      file.seek(SeekFrom::End(-1)).await?;
      Ok(file.read_u8().await.map_or(true, |v| v != 10))
    } else {
      // Empty files don't need an extra \n
      Ok(false)
    }
  }

  pub async fn open(&self) -> Result<RsonlDB<Opened>, Error> {
    let mut file = OpenOptions::new()
      .read(true)
      .write(true)
      .open(self.filename.to_owned())
      .await?;

    // Read the entire file. This also puts the cursor at the end, so we can start writing
    let entries = self.parse_entries(&mut file).await?;

    // Check if the file ends with \n
    let mut needs_lf = self.check_lf(&mut file).await?;

    // Pass some options to the write thread
    let throttle_interval: u128 = self.options.throttle_fs.interval_ms.into();
    let max_buffered_commands: usize = self.options.throttle_fs.max_buffered_commands.into();

    // We keep two references to the write backlog, one for putting entries in, one for reading it in the BG thread
    let write_backlog = Arc::new(Mutex::new(Vec::<String>::new()));
    let write_backlog2 = write_backlog.clone();

    let (tx, mut rx) = mpsc::channel(32);
    let thread = tokio::spawn(async move {
      let mut writer = BufWriter::new(file);
      let idle_duration = Duration::from_millis(20);

      let mut last_write = Instant::now();

      loop {
        let command = time::timeout(idle_duration, rx.recv()).await;
        let got_stop_command = match command {
          Ok(Some(Command::Stop)) => true,
          _ => false,
        };

        let mut must_write = got_stop_command
          || Instant::now().duration_since(last_write).as_millis() >= throttle_interval;

        // Grab all entries from the backlog
        let backlog: Vec<String> = {
          let mut write_backlog2 = write_backlog2.lock().unwrap();

          if !must_write && write_backlog2.len() > 0 && write_backlog2.len() > max_buffered_commands
          {
            must_write = true;
          }

          // If nothing needs to be written, wait for the next iteration
          if !must_write {
            continue;
          }

          write_backlog2.splice(.., []).collect()
        };

        // And print them
        for str in backlog.iter() {
          if str == "" {
            // Truncate the file
            writer.seek(SeekFrom::Start(0)).await.unwrap();
            writer.get_ref().set_len(0).await.unwrap();
            needs_lf = false;
          } else {
            if needs_lf {
              writer.write(b"\n").await.unwrap();
            }
            writer.write(str.as_bytes()).await.unwrap();
            writer.write(b"\n").await.unwrap();
            needs_lf = false;
          }
        }

        // Remember if we wrote something
        if backlog.len() > 0 {
          last_write = Instant::now();
        }

        if got_stop_command {
          break;
        }
      }

      // If we're stopping, make sure everything gets written
      writer.flush().await.unwrap();
    });

    // Change the state to Opened
    Ok(RsonlDB {
      filename: self.filename.to_owned(),
      options: self.options.clone(),
      state: Opened {
        entries,
        thread: Box::new(thread),
        tx,
        write_backlog,
      },
    })
  }
}

impl RsonlDB<Opened> {
  pub async fn close(&mut self) -> Result<RsonlDB<Closed>, Error> {
    // End the write thread and wait for it to end
    self.state.tx.send(Command::Stop).await.unwrap();
    self.state.thread.as_mut().await?;

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
    let str = serde_json::to_string(&Entry::Value {
      k: key.to_owned(),
      v: value.clone(),
    })
    .unwrap();

    self.state.entries.insert(key, MapValue::Raw(value));

    let mut write_backlog = self.state.write_backlog.lock().unwrap();
    write_backlog.push(str);
  }

  pub fn set_stringified(&mut self, key: String, value: String) {
    let str = format!(
      "{{\"k\":{},\"v\":{}}}",
      serde_json::to_string(&key).unwrap(),
      value
    );

    self
      .state
      .entries
      .insert(key, MapValue::Stringified(value.clone()));

    let mut write_backlog = self.state.write_backlog.lock().unwrap();
    write_backlog.push(str);
  }

  pub fn delete(&mut self, key: String) -> bool {
    if !self.has(&key) {
      return false;
    };

    let str = serde_json::to_string(&Entry::Delete { k: key.to_owned() }).unwrap();

    self.state.entries.remove(&key);

    {
      let mut write_backlog = self.state.write_backlog.lock().unwrap();
      write_backlog.push(str);
    }

    true
  }

  pub fn clear(&mut self) {
    self.state.entries.clear();

    // Clear anything on the write backlog, it is obsolete now
    let mut write_backlog = self.state.write_backlog.lock().unwrap();
    write_backlog.clear();
    write_backlog.push("".to_owned());
  }

  pub fn has(&self, key: &String) -> bool {
    self.state.entries.contains_key(key)
  }

  pub fn get(&self, key: &String) -> Option<serde_json::Value> {
    match self.state.entries.get(key) {
      Some(MapValue::Raw(val)) => Some(val.clone()),
      Some(MapValue::Stringified(str)) => Some(serde_json::Value::from_str(&str.clone()).unwrap()),
      None => None,
    }
  }

  pub fn size(&self) -> usize {
    self.state.entries.len()
  }

  pub fn keys(&self) -> std::collections::btree_map::Keys<String, MapValue> {
    self.state.entries.keys()
  }

  pub fn entries(&self) -> std::collections::btree_map::Iter<String, MapValue> {
    self.state.entries.iter()
  }
}
