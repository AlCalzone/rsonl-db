#[derive(Debug, Clone, Builder)]
#[builder(default)]
pub struct DBOptions {
  pub(crate) ignore_read_errors: bool,
  // reviver?: (key: string, value: any) => V;
  // serializer?: (key: string, value: V) => any;
  pub(crate) auto_compress: AutoCompressOptions,
  pub(crate) throttle_fs: ThrottleFSOptions,
  pub(crate) lockfile_directory: String,
  pub(crate) index_paths: Vec<String>,
}

impl Default for DBOptions {
  fn default() -> Self {
    Self {
      ignore_read_errors: false,
      auto_compress: AutoCompressOptions::default(),
      throttle_fs: ThrottleFSOptions::default(),
      lockfile_directory: ".".to_owned(),
      index_paths: Vec::new(),
    }
  }
}

#[derive(Debug, Clone, Builder)]
#[builder(default)]
pub struct AutoCompressOptions {
  pub(crate) size_factor: u32,
  pub(crate) size_factor_min_size: u32,
  pub(crate) interval_ms: u32,
  pub(crate) interval_min_changes: u32,
  pub(crate) on_close: bool,
  pub(crate) on_open: bool,
}

impl Default for AutoCompressOptions {
  fn default() -> Self {
    Self {
      size_factor: 0,
      size_factor_min_size: 0,
      interval_ms: 0,
      interval_min_changes: 1,
      on_close: false,
      on_open: false,
    }
  }
}

#[derive(Debug, Clone, Builder)]
#[builder(default)]
pub struct ThrottleFSOptions {
  pub(crate) interval_ms: u32,
  pub(crate) max_buffered_commands: usize,
}

impl Default for ThrottleFSOptions {
  fn default() -> Self {
    Self {
      interval_ms: 0,
      max_buffered_commands: usize::MAX,
    }
  }
}
