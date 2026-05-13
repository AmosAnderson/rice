use std::collections::HashMap;
use std::fmt;

use crate::error::RuntimeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasicType {
    Numeric,
    String,
    UserDefined(String),
}

#[derive(Debug, Clone)]
pub enum Value {
    Numeric(f64),
    Str(String),
    Record {
        type_name: String,
        fields: HashMap<String, Value>,
    },
}

impl Value {
    pub fn default_for(ty: BasicType) -> Value {
        match ty {
            BasicType::Numeric => Value::Numeric(0.0),
            BasicType::String => Value::Str(String::new()),
            BasicType::UserDefined(_) => {
                panic!(
                    "default_for(UserDefined) requires type definition context; use Interpreter::create_default_record"
                )
            }
        }
    }

    pub fn get_type(&self) -> BasicType {
        match self {
            Value::Numeric(_) => BasicType::Numeric,
            Value::Str(_) => BasicType::String,
            Value::Record { type_name, .. } => BasicType::UserDefined(type_name.clone()),
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Value::Numeric(_))
    }

    pub fn to_f64(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Numeric(n) => Ok(*n),
            Value::Str(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert string to number".into(),
            }),
            Value::Record { .. } => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert record to number".into(),
            }),
        }
    }

    pub fn to_i64(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Numeric(n) => Ok(*n as i64),
            Value::Str(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert string to integer".into(),
            }),
            Value::Record { .. } => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert record to integer".into(),
            }),
        }
    }

    pub fn to_string_val(&self) -> Result<String, RuntimeError> {
        match self {
            Value::Str(s) => Ok(s.clone()),
            _ => Err(RuntimeError::TypeMismatch {
                msg: "expected string".into(),
            }),
        }
    }

    pub fn coerce_to(&self, ty: BasicType) -> Result<Value, RuntimeError> {
        match ty {
            BasicType::Numeric => Ok(Value::Numeric(self.to_f64()?)),
            BasicType::String => Ok(Value::Str(self.to_string_val()?)),
            BasicType::UserDefined(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot coerce to user-defined type".into(),
            }),
        }
    }

    /// ANSI BASIC PRINT formatting:
    /// Numbers print without leading/trailing spaces.
    /// Strings: printed as-is.
    pub fn format_for_print(&self) -> String {
        match self {
            Value::Numeric(n) => format_number(*n),
            Value::Str(s) => s.clone(),
            Value::Record { type_name, .. } => format!("[{type_name}]"),
        }
    }

    /// WRITE# formatting: no leading/trailing spaces on numbers, strings get quoted by caller.
    pub fn format_for_write(&self) -> String {
        match self {
            Value::Numeric(n) => format_number(*n),
            Value::Str(s) => s.clone(),
            Value::Record { type_name, .. } => format!("[{type_name}]"),
        }
    }

    pub fn default_for_type(ty: Option<&BasicType>) -> Value {
        match ty {
            Some(t) => match t {
                BasicType::UserDefined(_) => Value::Numeric(0.0),
                other => Value::default_for(other.clone()),
            },
            None => Value::Numeric(0.0),
        }
    }

    /// Coerce to a target type, falling back to default on failure.
    pub fn coerce_to_type(&self, ty: &BasicType) -> Value {
        self.coerce_to(ty.clone())
            .unwrap_or_else(|_| Value::default_for_type(Some(ty)))
    }

    pub fn is_truthy(&self) -> Result<bool, RuntimeError> {
        match self {
            Value::Numeric(n) => Ok(*n != 0.0),
            Value::Str(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot use string as boolean".into(),
            }),
            Value::Record { .. } => Err(RuntimeError::TypeMismatch {
                msg: "cannot use record as boolean".into(),
            }),
        }
    }
}

fn format_number(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Numeric(n) => write!(f, "{n}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Record { type_name, .. } => write!(f, "[{type_name}]"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Str(a), Value::Str(b)) => a == b,
            (
                Value::Record {
                    type_name: ta,
                    fields: fa,
                },
                Value::Record {
                    type_name: tb,
                    fields: fb,
                },
            ) => ta == tb && fa == fb,
            (Value::Numeric(a), Value::Numeric(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Str(a), Value::Str(b)) => a.partial_cmp(b),
            (Value::Numeric(a), Value::Numeric(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}
