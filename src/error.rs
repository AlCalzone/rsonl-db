macro_rules! jserr {
  ($($msg:tt)*) => {
    napi::Error::from_reason(format!($($msg)*))
  };
}
