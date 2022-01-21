use filetime::FileTime;
use std::{
  fs,
  io::Error,
  path::{Path, PathBuf},
  time::SystemTime,
};

pub(crate) struct Lockfile {
  path: PathBuf,
  stale_interval_ms: u128,
  mtime: Option<FileTime>,
}

pub(crate) enum CheckResult {
  NoLock,
  Stale,
  Active(FileTime),
  Unknown,
}

impl Drop for Lockfile {
    fn drop(&mut self) {
        self.release();
    }
}

impl Lockfile {
  pub fn new(path: impl AsRef<Path>, stale_interval_ms: u128) -> Self {
    Self {
      path: path.as_ref().to_owned(),
      stale_interval_ms,
      mtime: None,
    }
  }

  pub fn get_stale_interval_ms(&self) -> u128 {
    self.stale_interval_ms
  }

  pub fn lock(&mut self) -> Result<(), Error> {
    match self.check() {
      CheckResult::NoLock => self.create_lock(),
      CheckResult::Stale => self.update_lock(),
      CheckResult::Active(_) => Err(Error::new(std::io::ErrorKind::Other, "Lockfile is in use")),
      CheckResult::Unknown => Err(Error::new(
        std::io::ErrorKind::Other,
        "Could not acquire lockfile",
      )),
    }
  }

  pub fn check(&mut self) -> CheckResult {
    if let Ok(meta) = fs::metadata(&self.path) {
      // File/Directory exists, check mtime
      let mtime = match meta.modified() {
        Ok(f) => f,
        _ => return CheckResult::Unknown,
      };
      let elapsed = match SystemTime::now().duration_since(mtime) {
        Ok(d) => d,
        _ => return CheckResult::Unknown,
      };
      if elapsed.as_millis() > self.stale_interval_ms {
        // stale, we can re-acquire it
        CheckResult::Stale
      } else {
        CheckResult::Active(FileTime::from(mtime))
      }
    } else {
      CheckResult::NoLock
    }
  }

  fn create_lock(&mut self) -> Result<(), Error> {
    fs::create_dir_all(&self.path)?;
    // And remember the timestamp
    let meta = fs::metadata(&self.path)?;
    let mtime = meta.modified()?;
    self.mtime = Some(mtime.into());
    Ok(())
  }

  fn update_lock(&mut self) -> Result<(), Error> {
    let now = FileTime::now();
    filetime::set_file_times(&self.path, now, now)?;
    self.mtime = Some(now.into());
    Ok(())
  }

  pub fn release(&mut self) {
    if let Some(self_mtime) = self.mtime {
      if let Ok(meta) = fs::metadata(&self.path) {
        // File/Directory exists, check mtime
        if let Ok(mtime) = meta.modified() {
          if FileTime::from(mtime) == self_mtime {
            // Our lock, release it
            fs::remove_dir(&self.path).ok();
          }
        }
      }
    }
    self.mtime = None;
  }

  pub fn update(&mut self) -> Result<(), Error> {
    match self.check() {
      CheckResult::NoLock => self.create_lock(),
      CheckResult::Stale => self.update_lock(),
      CheckResult::Active(mtime) => {
        if let Some(self_time) = self.mtime {
          if self_time != mtime {
            return Err(Error::new(
              std::io::ErrorKind::Other,
              "Lockfile was compromised",
            ));
          }
        }
        self.update_lock()
      }
      CheckResult::Unknown => Err(Error::new(
        std::io::ErrorKind::Other,
        "Could not update lockfile",
      )),
    }
  }
}
