use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::ReplError;

use super::value::{MatchObject, Value};

pub type ReplState = HashMap<String, StoredValue>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", content = "v")]
pub enum StoredValue {
    None,
    Bool(bool),
    Int(i64),
    Str(String),
    BytesB64(String),
    List(Vec<StoredValue>),
    Dict(HashMap<String, StoredValue>),
    Match {
        groups: Vec<String>,
        #[serde(default)]
        span_start: usize,
        #[serde(default)]
        span_end: usize,
    },
}

impl StoredValue {
    pub fn to_value(&self) -> Result<Value, ReplError> {
        match self {
            StoredValue::None => Ok(Value::None),
            StoredValue::Bool(b) => Ok(Value::Bool(*b)),
            StoredValue::Int(i) => Ok(Value::Int(*i)),
            StoredValue::Str(s) => Ok(Value::Str(s.clone())),
            StoredValue::BytesB64(s) => {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(s.as_bytes())
                    .map_err(|e| ReplError::ValueError(e.to_string()))?;
                Ok(Value::Bytes(bytes))
            }
            StoredValue::List(xs) => {
                let mut out = Vec::with_capacity(xs.len());
                for x in xs {
                    out.push(x.to_value()?);
                }
                Ok(Value::List(out))
            }
            StoredValue::Dict(m) => {
                let mut out = std::collections::BTreeMap::new();
                for (k, v) in m {
                    out.insert(k.clone(), v.to_value()?);
                }
                Ok(Value::Dict(out))
            }
            StoredValue::Match {
                groups,
                span_start,
                span_end,
            } => Ok(Value::Match(MatchObject {
                groups: groups.clone(),
                span_start: *span_start,
                span_end: *span_end,
            })),
        }
    }
}

pub fn try_from_value(v: &Value) -> Option<StoredValue> {
    match v {
        Value::None => Some(StoredValue::None),
        Value::Bool(b) => Some(StoredValue::Bool(*b)),
        Value::Int(i) => Some(StoredValue::Int(*i)),
        Value::Str(s) => Some(StoredValue::Str(s.clone())),
        Value::Bytes(b) => {
            use base64::Engine;
            let s = base64::engine::general_purpose::STANDARD.encode(b);
            Some(StoredValue::BytesB64(s))
        }
        Value::List(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(try_from_value(x)?);
            }
            Some(StoredValue::List(out))
        }
        Value::Dict(m) => {
            let mut out: HashMap<String, StoredValue> = HashMap::new();
            for (k, v) in m {
                out.insert(k.clone(), try_from_value(v)?);
            }
            Some(StoredValue::Dict(out))
        }
        Value::Match(m) => Some(StoredValue::Match {
            groups: m.groups.clone(),
            span_start: m.span_start,
            span_end: m.span_end,
        }),
        // We don't persist functions/modules across CLI calls yet.
        Value::UserFunc(_) | Value::Callable(_) | Value::Module(_) => None,
    }
}
