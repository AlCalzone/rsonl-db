use napi_derive::napi;

use crate::db_options::{DBOptions, DBOptionsBuilder, ThrottleFSOptionsBuilder};

#[napi(object, js_name = "JsonlDBOptions")]
pub struct JsonlDBOptions {
  #[napi(js_name = "ignoreReadErrors")]
  pub ignore_read_errors: Option<bool>,
  #[napi(js_name = "throttleFS")]
  pub throttle_fs: Option<JsonlDBOptionsThrottleFS>,
}

#[napi(object, js_name = "JsonlDBOptionsThrottleFS")]
pub struct JsonlDBOptionsThrottleFS {
  #[napi(js_name = "intervalMs")]
  pub interval_ms: u32,
  #[napi(js_name = "maxBufferedCommands")]
  pub max_buffered_commands: Option<u32>,
}

impl Default for JsonlDBOptions {
  fn default() -> Self {
    Self {
      ignore_read_errors: None,
      throttle_fs: None,
    }
  }
}

impl Into<DBOptions> for JsonlDBOptions {
  fn into(self) -> DBOptions {
    let mut ret = DBOptionsBuilder::default();

    if let Some(ignore_read_errors) = self.ignore_read_errors {
      ret.ignore_read_errors(ignore_read_errors);
    }

    if let Some(opts) = self.throttle_fs {
      let mut throttle = ThrottleFSOptionsBuilder::default();
      throttle.interval_ms(opts.interval_ms);
      if let Some(max_buf) = opts.max_buffered_commands {
        throttle.max_buffered_commands(max_buf as usize);
      }
      ret.throttle_fs(throttle.build().unwrap());
    }

    ret.build().unwrap()
  }
}

impl Into<DBOptions> for Option<JsonlDBOptions> {
  fn into(self) -> DBOptions {
    return self.unwrap_or_default().into();
  }
}
