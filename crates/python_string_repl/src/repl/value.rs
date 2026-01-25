use std::collections::BTreeMap;
use std::fmt;

use crate::error::ReplError;

#[derive(Clone, PartialEq)]
pub enum Value {
    None,
    Bool(bool),
    Int(i64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Dict(BTreeMap<String, Value>),
    Match(MatchObject),
    UserFunc(UserFunc),
    Callable(Callable),
    Module(Module),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => write!(f, "None"),
            Value::Bool(v) => write!(f, "Bool({v})"),
            Value::Int(v) => write!(f, "Int({v})"),
            Value::Str(v) => write!(f, "Str({:?})", v),
            Value::Bytes(v) => write!(f, "Bytes(len={})", v.len()),
            Value::List(v) => write!(f, "List(len={})", v.len()),
            Value::Dict(v) => write!(f, "Dict(len={})", v.len()),
            Value::Match(_) => write!(f, "Match(...)"),
            Value::UserFunc(u) => write!(f, "UserFunc({})", u.name),
            Value::Callable(c) => write!(f, "Callable({:?})", c),
            Value::Module(m) => write!(f, "Module({})", m.name),
        }
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::None => "None",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Str(_) => "str",
            Value::Bytes(_) => "bytes",
            Value::List(_) => "list",
            Value::Dict(_) => "dict",
            Value::Match(_) => "match",
            Value::UserFunc(_) => "function",
            Value::Callable(_) => "callable",
            Value::Module(_) => "module",
        }
    }

    pub fn as_str(&self) -> Result<&str, ReplError> {
        match self {
            Value::Str(s) => Ok(s.as_str()),
            _ => Err(ReplError::TypeError(format!(
                "expected str, got {}",
                self.type_name()
            ))),
        }
    }

    pub fn as_bytes(&self) -> Result<&[u8], ReplError> {
        match self {
            Value::Bytes(b) => Ok(b.as_slice()),
            _ => Err(ReplError::TypeError(format!(
                "expected bytes, got {}",
                self.type_name()
            ))),
        }
    }

    pub fn to_bool(&self) -> bool {
        match self {
            Value::None => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Str(s) => !s.is_empty(),
            Value::Bytes(b) => !b.is_empty(),
            Value::List(v) => !v.is_empty(),
            Value::Dict(m) => !m.is_empty(),
            Value::Match(_) => true,
            Value::UserFunc(_) => true,
            Value::Callable(_) => true,
            Value::Module(_) => true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchObject {
    pub groups: Vec<String>, // group(0) is the full match
    pub span_start: usize,
    pub span_end: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UserFunc {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<rustpython_parser::ast::Stmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Callable {
    Module { module: String, attr: String },
    BytesDecode { bytes: Vec<u8> },
    StrStrip { s: String },
    StrLower { s: String },
    StrFind { s: String },
    MatchGroup { m: MatchObject },
}
