use napi::{
  bindgen_prelude::{FromNapiValue, ToNapiValue},
  JsObject, Result,
};
use serde_json::Value;

pub enum JsValue {
  Primitive(Value),
  Object(JsObject),
}

impl ToNapiValue for JsValue {
  unsafe fn to_napi_value(
    env: napi::sys::napi_env,
    val: Self,
  ) -> napi::Result<napi::sys::napi_value> {
    match val {
      JsValue::Primitive(v) => ToNapiValue::to_napi_value(env, v),
      JsValue::Object(o) => ToNapiValue::to_napi_value(env, o),
    }
  }
}

pub(crate) unsafe fn value_to_js_object(
  env: napi::sys::napi_env,
  value: serde_json::Value,
) -> Result<JsObject> {
  let native = ToNapiValue::to_napi_value(env, value)?;
  let js_object = FromNapiValue::from_napi_value(env, native)?;
  Ok(js_object)
}
