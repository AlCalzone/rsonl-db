use napi_derive::napi;

use crate::db_options::{
  AutoCompressOptionsBuilder, DBOptions, DBOptionsBuilder, ThrottleFSOptionsBuilder,
};

#[napi(object, js_name = "JsonlDBOptions")]
pub struct JsonlDBOptions {
  #[napi]
  pub ignore_read_errors: Option<bool>,
  #[napi(js_name = "throttleFS")]
  pub throttle_fs: Option<JsonlDBOptionsThrottleFS>,
  #[napi]
  pub auto_compress: Option<JsonlDBOptionsAutoCompress>,
  #[napi]
  pub lockfile_directory: Option<String>,
  #[napi]
  pub index_paths: Option<Vec<String>>,
}

#[napi(object, js_name = "JsonlDBOptionsThrottleFS")]
pub struct JsonlDBOptionsThrottleFS {
  #[napi]
  pub interval_ms: u32,
  #[napi]
  pub max_buffered_commands: Option<u32>,
}

#[napi(object, js_name = "JsonlDBOptionsAutoCompress")]
pub struct JsonlDBOptionsAutoCompress {
  #[napi]
  pub size_factor: Option<u32>,
  #[napi]
  pub size_factor_minimum_size: Option<u32>,
  #[napi]
  pub interval_ms: Option<u32>,
  #[napi]
  pub interval_min_changes: Option<u32>,
  #[napi]
  pub on_close: Option<bool>,
  #[napi]
  pub on_open: Option<bool>,
}

impl Default for JsonlDBOptions {
  fn default() -> Self {
    Self {
      ignore_read_errors: None,
      throttle_fs: None,
      auto_compress: None,
      lockfile_directory: None,
      index_paths: None,
    }
  }
}

impl Into<DBOptions> for JsonlDBOptions {
  fn into(self) -> DBOptions {
    let mut ret = DBOptionsBuilder::default();

    if let Some(ignore_read_errors) = self.ignore_read_errors {
      ret.ignore_read_errors(ignore_read_errors);
    }

    if let Some(opts) = self.auto_compress {
      let mut compress = AutoCompressOptionsBuilder::default();
      if let Some(size_factor) = opts.size_factor {
        compress.size_factor(size_factor);
      }
      if let Some(size_factor_min_size) = opts.size_factor_minimum_size {
        compress.size_factor_min_size(size_factor_min_size);
      }
      if let Some(interval_ms) = opts.interval_ms {
        compress.interval_ms(interval_ms);
      }
      if let Some(interval_min_changes) = opts.interval_min_changes {
        compress.interval_min_changes(interval_min_changes);
      }
      if let Some(on_close) = opts.on_close {
        compress.on_close(on_close);
      }
      if let Some(on_open) = opts.on_open {
        compress.on_open(on_open);
      }

      ret.auto_compress(compress.build().unwrap());
    }

    if let Some(opts) = self.throttle_fs {
      let mut throttle = ThrottleFSOptionsBuilder::default();
      throttle.interval_ms(opts.interval_ms);
      if let Some(max_buf) = opts.max_buffered_commands {
        throttle.max_buffered_commands(max_buf as usize);
      }
      ret.throttle_fs(throttle.build().unwrap());
    }

    if let Some(lockfile_directory) = self.lockfile_directory {
      ret.lockfile_directory(lockfile_directory);
    }

    if let Some(index_paths) = self.index_paths {
      ret.index_paths(index_paths);
    }

    ret.build().unwrap()
  }
}

impl Into<DBOptions> for Option<JsonlDBOptions> {
  fn into(self) -> DBOptions {
    return self.unwrap_or_default().into();
  }
}
