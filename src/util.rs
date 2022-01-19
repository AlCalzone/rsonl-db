use std::io::{SeekFrom, Error};
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncReadExt};

pub(crate) async fn file_needs_lf(file: &mut File) -> Result<bool, Error> {
  if file.metadata().await?.len() > 0 {
    file.seek(SeekFrom::End(-1)).await?;
    Ok(file.read_u8().await.map_or(true, |v| v != 10))
  } else {
    // Empty files don't need an extra \n
    Ok(false)
  }
}
