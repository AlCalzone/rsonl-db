use napi::{JsObject, JsString};
use std::io::{Error, SeekFrom};
use std::path::{Path, PathBuf};
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

pub(crate) fn safe_parent(p: impl AsRef<Path>) -> Option<PathBuf> {
  match p.as_ref().parent() {
    None => None,
    Some(x) => {
      if x.as_os_str().is_empty() {
        Some(Path::new(".").to_owned())
      } else {
        Some(x.to_owned())
      }
    }
  }
}

pub(crate) fn replace_dirname(
  path: impl AsRef<Path>,
  dirname: impl AsRef<Path>,
) -> Result<PathBuf, Error> {
  let filename = Path::new(path.as_ref().file_name().unwrap());
  let basename = path.as_ref().parent().unwrap();
  let ret: PathBuf = [basename, dirname.as_ref(), filename].iter().collect();
  Ok(ret)
}

pub(crate) fn obj_matches_filter(o: &JsObject, filter: &Option<String>) -> bool {
  let filter = filter.as_ref().map_or(None, |v| v.split_once('='));
  if let Some((filter_prop, filter_val)) = filter {
    let t = &o
      .get_named_property::<JsString>(filter_prop)
      .and_then(|t| t.into_utf8());
    let t = match t {
      Ok(t) => t.as_str(),
      Err(_) => return false,
    };
    match t {
      Ok(t) => return t == filter_val,
      Err(_) => return false,
    }
  } else {
    // no filter
    return true;
  }
}
