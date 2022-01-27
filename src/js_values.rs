use napi::{
  bindgen_prelude::ToNapiValue,
  Env, JsObject, Result,
};
use serde_json::{Map, Value};

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

pub(crate) fn map_to_object(env: Env, map: Map<String, Value>) -> Result<JsObject> {
  let mut obj = env.create_object()?;

  for (k, v) in map.into_iter() {
    obj.set(k, v)?;
  }

  Ok(obj)
}

pub(crate) fn vec_to_array(env: Env, vec: Vec<Value>) -> Result<JsObject> {
  let mut obj = env.create_array_with_length(vec.len())?;

  let i: u32 = 0;
  for v in vec.into_iter() {
    obj.set(format!("{}", &i), v)?;
  }

  Ok(obj)
}
