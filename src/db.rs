use std::io::{Error, SeekFrom};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use indexmap::IndexMap;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::mpsc::{self};

use tokio::time::{self, Instant};

use serde::{Deserialize, Serialize};

use crate::bg_thread::{ThreadHandle, Command};
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

impl Into<String> for MapValue {
  fn into(self) -> String {
    match self {
      MapValue::Stringified(str) => str,
      MapValue::Raw(v) => serde_json::to_string(&v).unwrap(),
    }
  }
}

fn format_line(key: &str, val: MapValue) -> String {
  format!(
    "{{\"k\":{},\"v\":{}}}",
    serde_json::to_string(key).unwrap(),
    <MapValue as Into<String>>::into(val)
  )
}

pub(crate) struct RsonlDB<S: DBState> {
  pub filename: PathBuf,
  options: DBOptions,
  pub state: S,
}

// Data that's only present in certain DB states
pub(crate) struct Closed;

pub(crate) struct Opened {
  entries: Mutex<IndexMap<String, MapValue>>,
  // BG thread handles
  write_thread: Option<ThreadHandle<()>>,
  backup_thread: Option<ThreadHandle<Result<(), Error>>>,
  // statistics
  uncompressed_size: usize,
  changes_since_compress: usize,
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

impl Opened {
  fn write_line(&mut self, line: &str) -> Result<(), Error> {
    // Write into the backlog of whichever thread is currently running
    if let Some(thread) = self.write_thread.as_mut() {
      thread.push_backlog(line);
    }
    if let Some(thread) = self.backup_thread.as_mut() {
      thread.push_backlog(line);
    }

    if line == "" {
      self.uncompressed_size = 0;
    } else {
      self.uncompressed_size += 1;
    }
    self.changes_since_compress += 1;

    Ok(())
  }

  async fn stop_threads(&mut self) -> Result<(), Error> {
    // End the all threads and wait for them to end
    if let Some(thread) = self.write_thread.as_mut() {
      thread.stop_and_join().await?;
    }
    if let Some(thread) = self.backup_thread.as_mut() {
      thread.stop_and_join().await?;
    }
    Ok(())
  }
}


impl RsonlDB<Closed> {
  pub fn new(filename: PathBuf, options: DBOptions) -> Self {
    RsonlDB {
      filename,
      options,
      state: Closed,
    }
  }

  async fn parse_entries(&self, file: &mut File) -> Result<IndexMap<String, MapValue>, Error> {
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
      .create(true)
      .read(true)
      .write(true)
      .open(self.filename.to_owned())
      .await?;

    // Read the entire file. This also puts the cursor at the end, so we can start writing
    let entries = self.parse_entries(&mut file).await?;

    // Check if the file ends with \n
    let needs_lf = self.check_lf(&mut file).await?;

    // Pass some options to the write thread
    let throttle_interval: u128 = self.options.throttle_fs.interval_ms.into();
    let max_buffered_commands: usize = self.options.throttle_fs.max_buffered_commands.into();

    // We keep two references to the write backlog, one for putting entries in, one for reading it in the BG thread
    let write_backlog_in = Arc::new(Mutex::new(Vec::<String>::new()));
    let write_backlog_out = write_backlog_in.clone();

    // Start the write thread
    let (write_tx, write_rx) = mpsc::channel(32);
    let write_thread = tokio::spawn(async move {
      write_thread(
        file,
        write_rx,
        throttle_interval,
        write_backlog_out,
        max_buffered_commands,
        needs_lf,
      )
      .await;
    });

    // Now change the state to Opened
    Ok(RsonlDB {
      filename: self.filename.to_owned(),
      options: self.options.clone(),
      state: Opened {
        uncompressed_size: entries.len(),
        changes_since_compress: 0,
        entries: Mutex::new(entries),
        write_thread: Some(ThreadHandle {
          backlog: write_backlog_in,
          thread: Some(Box::new(write_thread)),
          tx: Some(write_tx),
        }),
        backup_thread: None,
      },
    })
  }
}

async fn write_thread(
  file: File,
  mut write_rx: mpsc::Receiver<Command>,
  throttle_interval: u128,
  write_backlog: Arc<Mutex<Vec<String>>>,
  max_buffered_commands: usize,
  mut needs_lf: bool,
) {
  let idle_duration = Duration::from_millis(20);
  let mut last_write = Instant::now();

  let mut writer = BufWriter::new(file);

  loop {
    let command = time::timeout(idle_duration, write_rx.recv()).await;
    let got_stop_command = command == Ok(Some(Command::Stop));

    let mut must_write = got_stop_command
      || Instant::now().duration_since(last_write).as_millis() >= throttle_interval;

    // Grab all entries from the backlog
    let backlog: Vec<String> = {
      let mut write_backlog = write_backlog.lock().unwrap();

      if !must_write && write_backlog.len() > 0 && write_backlog.len() > max_buffered_commands {
        must_write = true;
      }

      // If nothing needs to be written, wait for the next iteration
      if !must_write {
        continue;
      }

      write_backlog.splice(.., []).collect()
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
  writer.flush().await.unwrap();
}

impl RsonlDB<Opened> {
  pub async fn close(&mut self) -> Result<RsonlDB<Closed>, Error> {
    // End the all threads and wait for them to end
    self.state.stop_threads().await?;

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

    {
      let mut entries = self.state.entries.lock().unwrap();
      entries.insert(key, MapValue::Raw(value));
    }
    self.state.write_line(&str).unwrap();
  }

  pub fn set_stringified(&mut self, key: String, value: String) {
    let str = format!(
      "{{\"k\":{},\"v\":{}}}",
      serde_json::to_string(&key).unwrap(),
      value
    );

    {
      let mut entries = self.state.entries.lock().unwrap();
      entries.insert(key, MapValue::Stringified(value.clone()));
    }
    self.state.write_line(&str).unwrap();
  }

  pub fn delete(&mut self, key: String) -> bool {
    if !self.has(&key) {
      return false;
    };

    let str = serde_json::to_string(&Entry::Delete { k: key.to_owned() }).unwrap();

    {
      let mut entries = self.state.entries.lock().unwrap();
      entries.remove(&key);
    }
    self.state.write_line(&str).unwrap();

    true
  }

  pub fn clear(&mut self) {
    {
      let mut entries = self.state.entries.lock().unwrap();
      entries.clear();
    }
    self.state.write_line("").unwrap();
  }

  pub fn has(&self, key: &String) -> bool {
    let entries = self.state.entries.lock().unwrap();
    entries.contains_key(key)
  }

  pub fn get(&self, key: &String) -> Option<serde_json::Value> {
    let entries = self.state.entries.lock().unwrap();
    match entries.get(key) {
      Some(MapValue::Raw(val)) => Some(val.clone()),
      Some(MapValue::Stringified(str)) => Some(serde_json::Value::from_str(&str.clone()).unwrap()),
      None => None,
    }
  }

  pub fn size(&self) -> usize {
    let entries = self.state.entries.lock().unwrap();
    entries.len()
  }

  pub fn all_keys(&self) -> Vec<String> {
    let entries = self.state.entries.lock().unwrap();
    entries.keys().cloned().collect()
  }

  // pub fn entries(&self) -> std::collections::btree_map::Iter<String, MapValue> {
  //   let entries = self.state.entries.lock().unwrap();
  //   entries.iter().clone()
  // }

  pub async fn dump(&mut self, filename: &str) -> Result<(), Error> {
    // Create the backlog first thing so we don't miss any writes while copying
    // We keep two references to the write backlog, one for putting entries in, one for reading it in the BG thread
    let backlog_in = Arc::new(Mutex::new(Vec::<String>::new()));
    let backlog_out = backlog_in.clone();

    self.state.backup_thread = Some(ThreadHandle {
      backlog: backlog_in,
      thread: None,
      tx: None,
    });
    let backup_thread_handle = self.state.backup_thread.as_mut().unwrap();

    // Create a copy of the internal map so we can move it to the bg thread
    let data = {
      let entries = self.state.entries.lock().unwrap();
      entries.clone()
    };

    let file = OpenOptions::new()
      .create(true)
      .write(true)
      .truncate(true)
      .open(filename)
      .await?;

    let write_thread = tokio::spawn(async move {
      let mut writer = BufWriter::new(file);

      // Print all items
      for (key, val) in data {
        writer.write(format_line(&key, val).as_bytes()).await?;
        writer.write(b"\n").await?;
      }

      // Then print whatever is left in the backlog
      // Grab all entries from the backlog
      let backlog: Vec<String> = {
        let mut backlog_out = backlog_out.lock().unwrap();
        backlog_out.splice(.., []).collect()
      };

      for str in backlog.iter() {
        if str == "" {
          // Truncate the file
          writer.seek(SeekFrom::Start(0)).await?;
          writer.get_ref().set_len(0).await?;
        } else {
          writer.write(str.as_bytes()).await?;
          writer.write(b"\n").await?;
        }
      }

      // And make sure everything is on disk
      writer.flush().await?;

      Ok(())
    });

    backup_thread_handle.thread = Some(Box::new(write_thread));
    backup_thread_handle.join().await?;

    self.state.backup_thread = None;

    Ok(())
  }
}
