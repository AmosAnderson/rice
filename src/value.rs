use std::collections::HashMap;
use std::fmt;

use crate::error::RuntimeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasicType {
    Numeric,
    Integer,
    Long,
    Single,
    Double,
    String,
    FixedLengthString(usize),
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

/// BASIC binary strings treat each character as one byte. Rust strings are UTF-8,
/// so packed binary helpers use Latin-1-style chars U+0000..U+00FF.
pub fn bytes_to_basic_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

pub fn basic_string_to_bytes(s: &str) -> Vec<u8> {
    s.chars().map(|c| (c as u32).min(0xFF) as u8).collect()
}

impl Value {
    pub fn default_for(ty: BasicType) -> Value {
        match ty {
            BasicType::Numeric
            | BasicType::Integer
            | BasicType::Long
            | BasicType::Single
            | BasicType::Double => Value::Numeric(0.0),
            BasicType::String | BasicType::FixedLengthString(_) => Value::Str(String::new()),
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
            Value::Numeric(n) => {
                // i64::MAX rounds up to 2^63 as f64, so the upper bound is exclusive.
                if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(n) {
                    return Err(RuntimeError::Overflow {
                        msg: "number is outside the integer range".into(),
                    });
                }
                Ok(*n as i64)
            }
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
            BasicType::Numeric
            | BasicType::Integer
            | BasicType::Long
            | BasicType::Single
            | BasicType::Double => Ok(Value::Numeric(self.to_f64()?)),
            BasicType::String | BasicType::FixedLengthString(_) => {
                Ok(Value::Str(self.to_string_val()?))
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_conversion_rejects_nonfinite_and_out_of_range_values() {
        for n in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            9_223_372_036_854_775_808.0,
            -9_223_372_036_854_777_856.0,
        ] {
            assert!(matches!(
                Value::Numeric(n).to_i64(),
                Err(RuntimeError::Overflow { .. })
            ));
        }
    }

    #[test]
    fn integer_conversion_preserves_truncation_and_valid_boundaries() {
        assert_eq!(Value::Numeric(1.9).to_i64().unwrap(), 1);
        assert_eq!(Value::Numeric(-1.9).to_i64().unwrap(), -1);
        assert_eq!(Value::Numeric(i64::MIN as f64).to_i64().unwrap(), i64::MIN);
        assert_eq!(
            Value::Numeric(9_223_372_036_854_774_784.0)
                .to_i64()
                .unwrap(),
            9_223_372_036_854_774_784,
        );
        assert!(matches!(
            Value::Str("1".into()).to_i64(),
            Err(RuntimeError::TypeMismatch { .. })
        ));
    }
}
