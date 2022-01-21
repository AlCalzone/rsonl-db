use std::{
  io::{Error, SeekFrom},
  path::Path,
  time::Duration,
};

use tokio::{
  fs::{self, File, OpenOptions},
  io::{AsyncSeekExt, AsyncWriteExt, BufWriter},
  sync::mpsc::Receiver,
  time::{self, error::Elapsed, Instant},
};

use crate::{
  bg_thread::Command,
  db_options::{AutoCompressOptions, DBOptions},
  lockfile::Lockfile,
  storage::{format_line, SharedStorage},
  util::{file_needs_lf, fsync_dir, safe_parent},
};

fn is_stop_cmd(cmd: Result<Option<Command>, Elapsed>) -> bool {
  match cmd {
    Ok(Some(Command::Stop)) => true,
    _ => false,
  }
}

fn need_to_compress_by_size(opts: &AutoCompressOptions, size: u32, uncompressed_size: u32) -> bool {
  if opts.size_factor == 0 {
    return false;
  }

  return uncompressed_size as u32 >= opts.size_factor_min_size
    && uncompressed_size as u32 >= opts.size_factor * size;
}

fn need_to_compress_by_time(
  opts: &AutoCompressOptions,
  last_compress: Instant,
  changes_since_compress: u32,
) -> bool {
  if opts.interval_ms == 0 {
    return false;
  }

  return changes_since_compress >= opts.interval_min_changes
    && Instant::now().duration_since(last_compress).as_millis() > opts.interval_ms as u128;
}

pub(crate) async fn persistence_thread(
  filename: &str,
  mut file: File,
  mut storage: SharedStorage,
  mut lock: Lockfile,
  mut rx: Receiver<Command>,
  opts: &DBOptions,
) -> Result<(), Error> {
  // Keep track of the write accesses
  let mut last_write = Instant::now();
  let throttle_interval = opts.throttle_fs.interval_ms as u128;
  let max_buffered_commands = opts.throttle_fs.max_buffered_commands;
  let mut last_lockfile_refresh = Instant::now();

  // And compression attempts
  let mut last_compress = Instant::now();
  let mut uncompressed_size: usize = storage.len();
  let mut changes_since_compress: usize = 0;

  // Open writer and make sure the file ends with LF
  let mut writer = {
    let needs_lf = file_needs_lf(&mut file).await.unwrap();
    let mut ret = BufWriter::new(file);
    if needs_lf {
      ret.write(b"\n").await.unwrap();
    }
    ret
  };

  let mut just_opened: bool = true;

  let idle_duration = Duration::from_millis(20);
  loop {

    // Refresh lockfile if necessary
    if Instant::now().duration_since(last_lockfile_refresh).as_millis() >= lock.get_stale_interval_ms() {
      lock.update()?;
      last_lockfile_refresh = Instant::now();
    }

    // Figure out what to do
    let command = if (just_opened && opts.auto_compress.on_open)
      || need_to_compress_by_size(
        &opts.auto_compress,
        storage.len() as u32,
        uncompressed_size as u32,
      )
      || need_to_compress_by_time(
        &opts.auto_compress,
        last_compress,
        changes_since_compress as u32,
      ) {
      // We need to compress, do it now!
      Ok(Some(Command::Compress { done: None }))
    } else {
      // If we don't have to compress, wait for a command
      time::timeout(idle_duration, rx.recv()).await
    };

    just_opened = false;

    // Figure out if there is something to do
    match command {
      Ok(Some(Command::Stop)) | Ok(None) | Err(_) => {
        // No command or we were asked to stop
        let stop = is_stop_cmd(command);

        // Write to disk if necessary
        if stop || Instant::now().duration_since(last_write).as_millis() >= throttle_interval {
          let backlog = storage.drain_backlog_if(|b| {
            b.len() > 0 && (throttle_interval == 0 || b.len() > max_buffered_commands)
          });

          if stop || backlog.len() > 0 {
            for str in backlog.iter() {
              if str == "" {
                // Truncate the file
                writer.seek(SeekFrom::Start(0)).await?;
                writer.get_ref().set_len(0).await?;
                // Now the DB size is effectively 0 and we have no "uncompressed" changes pending
                uncompressed_size = 0;
                changes_since_compress = 0;
              } else {
                writer.write(str.as_bytes()).await?;
                uncompressed_size += 1;
                changes_since_compress += 1;
              }
            }

            last_write = Instant::now();
          }
        }

        if stop {
          // Make sure everything is on disk
          writer.flush().await?;
          writer.get_ref().sync_all().await?;

          break;
        }
      }

      Ok(Some(Command::Compress { done })) => {
        // Compress the database
        let filename = filename.to_owned();
        let dump_filename = format!("{}.dump", &filename);
        let backup_filename = format!("{}.bak", &filename);
        let dirname = safe_parent(Path::new(&filename)).unwrap();

        // 1. Ensure the backup contains everything in the DB and backlog
        let write_backlog = storage.drain_backlog();
        for str in write_backlog.iter() {
          if str == "" {
            // Truncate the file
            writer.seek(SeekFrom::Start(0)).await?;
            writer.get_ref().set_len(0).await?;
            // Now the DB size is effectively 0 and we have no "uncompressed" changes pending
            uncompressed_size = 0;
            changes_since_compress = 0;
          } else {
            writer.write(str.as_bytes()).await?;
            uncompressed_size += 1;
            changes_since_compress += 1;
          }
        }
        // Make sure everything is on disk
        writer.flush().await?;
        writer.get_ref().sync_all().await?;

        // Close the file
        drop(writer);

        // 2. Create a dump, draining the backlog to avoid duplicate writes
        dump(&dump_filename, &mut storage, true).await?;

        // 3. Ensure there are no pending rename operations or file creations
        fsync_dir(&dirname).await?;

        // 4. Swap files around, then ensure the directory entries are written to disk
        fs::rename(&filename, &backup_filename).await?;
        fs::rename(&dump_filename, &filename).await?;
        fsync_dir(&dirname).await?;

        // 5. Delete backup
        fs::remove_file(&backup_filename).await?;

        // 6. open the main DB file again
        file = OpenOptions::new()
          .create(true)
          .read(true)
          .write(true)
          .open(&filename)
          .await?;
        writer = BufWriter::new(file);
        writer.seek(SeekFrom::End(0)).await?;
        // Any "new" data in the backlog will be written in the next iteration

        // Remember the new statistics
        uncompressed_size = storage.len();
        changes_since_compress = 0;
        last_compress = Instant::now();

        // invoke the callback
        if let Some(done) = done {
          done.notify_waiters();
        }
      }

      Ok(Some(Command::Dump { filename, done })) => {
        // Create a backup
        dump(&filename, &mut storage, false).await?;

        // invoke the callback
        done.notify_waiters();
      }
    }
  }

  Ok(())
}

async fn dump(
  filename: &str,
  storage: &mut SharedStorage,
  drain_backlog: bool,
) -> Result<(), Error> {
  let dump_file = OpenOptions::new()
    .create(true)
    .write(true)
    .truncate(true)
    .open(filename)
    .await?;

  let mut writer = BufWriter::new(dump_file);

  // Create a copy of the internal map so we only need to hold on to it very shortly
  // Also, remember how many entries were in the backlog. These are already part of
  // the map, so we don't need to append them later
  // and keep a consistent state
  let (data, backlog_len) = {
    let storage = storage.lock().unwrap();
    let entries = &storage.entries;
    let backlog = &storage.backlog;
    (entries.clone(), backlog.len())
  };

  // Print all items
  for (key, val) in data {
    writer.write(format_line(&key, val).as_bytes()).await?;
  }

  // And append any new entries in the backlog
  let backlog = if drain_backlog {
    storage.drain_backlog()
  } else {
    storage.lock().unwrap().backlog.clone()
  };
  for str in backlog.iter().skip(backlog_len) {
    if str == "" {
      // Truncate the file
      writer.seek(SeekFrom::Start(0)).await?;
      writer.get_ref().set_len(0).await?;
    } else {
      writer.write(str.as_bytes()).await?;
    }
  }

  // Make sure everything is on disk
  writer.flush().await?;
  writer.get_ref().sync_all().await?;

  Ok(())
}
