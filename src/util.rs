use std::io::{Error, SeekFrom};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub(crate) async fn file_needs_lf(file: &mut File) -> Result<bool, Error> {
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
) -> Result<(), Error> {
  #[cfg(not(target_os = "windows"))]
  {
    let file = File::open(dir).await?;
    file.sync_all().await?;
  }
  Ok(())
}

pub(crate) fn safe_parent(p: &Path) -> Option<&Path> {
  match p.parent() {
    None => None,
    Some(x) if x.as_os_str().is_empty() => Some(Path::new(".")),
    x => x,
  }
}
