use std::{io::SeekFrom, path::Path, time::Duration};

use tokio::{
  fs::{self, File, OpenOptions},
  io::{AsyncSeekExt, AsyncWriteExt, BufWriter},
  sync::mpsc::Receiver,
  time::{self, error::Elapsed, Instant},
};

use crate::{
  bg_thread::Command,
  db_options::{AutoCompressOptions, DBOptions},
  error::Result,
  lockfile::Lockfile,
  storage::{format_line, SharedStorage},
  util::{file_needs_lf, fsync_dir, parent_dir},
};

fn is_stop_cmd(cmd: std::result::Result<Option<Command>, Elapsed>) -> bool {
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
) -> Result<()> {
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
    let needs_lf = file_needs_lf(&mut file).await?;
    let mut ret = BufWriter::new(file);
    if needs_lf {
      ret.write(b"\n").await?;
    }
    ret
  };

  let mut just_opened: bool = true;

  let idle_duration = Duration::from_millis(20);
  loop {
    // Refresh lockfile if necessary
    if Instant::now()
      .duration_since(last_lockfile_refresh)
      .as_millis()
      >= lock.get_stale_interval_ms()
    {
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
        let journal_len = storage.journal_len();
        let should_write = journal_len > 0
          && (stop
            || Instant::now().duration_since(last_write).as_millis() >= throttle_interval
            || journal_len > max_buffered_commands);

        if should_write {
          let journal = storage.drain_journal();

          for str in journal {
            if str == "" {
              // Truncate the file
              writer.rewind().await?;
              writer.get_ref().set_len(0).await?;
              // Now the DB size is effectively 0 and we have no "uncompressed" changes pending
              uncompressed_size = 0;
              changes_since_compress = 0;
            } else {
              writer.write(str.as_bytes()).await?;
              writer.write(b"\n").await?;
              uncompressed_size += 1;
              changes_since_compress += 1;
            }
          }

          // Make sure everything is on disk
          writer.flush().await?;
          last_write = Instant::now();
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
        let dirname = parent_dir(Path::new(&filename))?;

        // 1. Ensure the backup contains everything in the DB and journal
        let write_journal = storage.drain_journal();
        for str in write_journal.iter() {
          if str == "" {
            // Truncate the file
            writer.seek(SeekFrom::Start(0)).await?;
            writer.get_ref().set_len(0).await?;
            // Now the DB size is effectively 0 and we have no "uncompressed" changes pending
            uncompressed_size = 0;
            changes_since_compress = 0;
          } else {
            writer.write(str.as_bytes()).await?;
            writer.write(b"\n").await?;
            uncompressed_size += 1;
            changes_since_compress += 1;
          }
        }
        // Make sure everything is on disk
        writer.flush().await?;
        writer.get_ref().sync_all().await?;

        // Close the file
        drop(writer);

        // 2. Create a dump, draining the journal to avoid duplicate writes
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
        // Any "new" data in the journal will be written in the next iteration

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

async fn dump(filename: &str, storage: &mut SharedStorage, drain_journal: bool) -> Result<()> {
  let dump_file = OpenOptions::new()
    .create(true)
    .write(true)
    .truncate(true)
    .open(filename)
    .await?;

  let mut writer = BufWriter::new(dump_file);

  // Render the compressed file in memory so we only need to lock the storage very shortly
  // Also, remember how many entries were in the journal. These are already part of
  // the map, so we don't need to append them later
  // and keep a consistent state
  let (dump, journal_len) = {
    let storage = storage.lock();
    let journal = &storage.journal;

    let dump: Vec<u8> = storage
      .entries
      .iter()
      .flat_map(|(key, val)| [format_line(key, val).as_bytes(), b"\n"].concat())
      .collect();
    (dump, journal.len())
  };

  // Print all items
  writer.write_all(dump.as_slice()).await?;

  // And append any new entries in the journal
  let journal = if drain_journal {
    storage.drain_journal()
  } else {
    storage.clone_journal()
  };
  for str in journal.iter().skip(journal_len) {
    if str == "" {
      // Truncate the file
      writer.seek(SeekFrom::Start(0)).await?;
      writer.get_ref().set_len(0).await?;
    } else {
      writer.write(str.as_bytes()).await?;
      writer.write(b"\n").await?;
    }
  }

  // Make sure everything is on disk
  writer.flush().await?;
  writer.get_ref().sync_all().await?;

  Ok(())
}
