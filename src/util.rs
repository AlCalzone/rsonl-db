use crate::error::{JsonlDBError, Result};
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub(crate) async fn file_needs_lf(file: &mut File) -> Result<bool> {
  if file.metadata().await?.len() > 0 {
    file.seek(SeekFrom::End(-1)).await?;
    Ok(file.read_u8().await.map_or(true, |v| v != 10))
  } else {
    // Empty files don't need an extra \n
    Ok(false)
  }
}

pub(crate) async fn fsync_dir(
  #[cfg_attr(target_os = "windows", allow(unused_variables))] dir: &Path,
) -> Result<()> {
  #[cfg(not(target_os = "windows"))]
  {
    let file = File::open(dir).await?;
    file.sync_all().await?;
  }
  Ok(())
}

pub(crate) fn parent_dir(p: impl AsRef<Path>) -> Result<PathBuf> {
  match p.as_ref().parent() {
    None => Err(JsonlDBError::io_error_from_reason(format!(
      "\"{}\" does not have a parent directory",
      &p.as_ref().to_str().unwrap_or("unknown dir")
    ))),
    Some(x) => {
      if x.as_os_str().is_empty() {
        Ok(Path::new(".").to_owned())
      } else {
        Ok(x.to_owned())
      }
    }
  }
}

pub(crate) fn replace_dirname(
  path: impl AsRef<Path>,
  dirname: impl AsRef<Path>,
) -> Option<PathBuf> {
  let filename = Path::new(path.as_ref().file_name()?);
  let basename = path.as_ref().parent()?;
  let ret: PathBuf = [basename, dirname.as_ref(), filename].iter().collect();
  Some(ret)
}
