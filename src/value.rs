use std::fmt;

use crate::error::RuntimeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicType {
    Integer,
    Long,
    Single,
    Double,
    String,
}

#[derive(Debug, Clone)]
pub enum Value {
    Integer(i64),
    Long(i64),
    Single(f64),
    Double(f64),
    Str(String),
}

impl Value {
    pub fn default_for(ty: BasicType) -> Value {
        match ty {
            BasicType::Integer => Value::Integer(0),
            BasicType::Long => Value::Long(0),
            BasicType::Single => Value::Single(0.0),
            BasicType::Double => Value::Double(0.0),
            BasicType::String => Value::Str(String::new()),
        }
    }

    pub fn default_for_suffix(suffix: Option<crate::token::TypeSuffix>) -> Value {
        match suffix {
            Some(s) => Value::default_for(s.to_basic_type()),
            None => Value::Integer(0),
        }
    }

    pub fn get_type(&self) -> BasicType {
        match self {
            Value::Integer(_) => BasicType::Integer,
            Value::Long(_) => BasicType::Long,
            Value::Single(_) => BasicType::Single,
            Value::Double(_) => BasicType::Double,
            Value::Str(_) => BasicType::String,
        }
    }

    pub fn is_numeric(&self) -> bool {
        !matches!(self, Value::Str(_))
    }

    pub fn to_f64(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Integer(n) => Ok(*n as f64),
            Value::Long(n) => Ok(*n as f64),
            Value::Single(n) => Ok(*n),
            Value::Double(n) => Ok(*n),
            Value::Str(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert string to number".into(),
            }),
        }
    }

    pub fn to_i64(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Integer(n) | Value::Long(n) => Ok(*n),
            Value::Single(n) => Ok(*n as i64),
            Value::Double(n) => Ok(*n as i64),
            Value::Str(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert string to integer".into(),
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
            BasicType::Integer => Ok(Value::Integer(self.to_i64()?)),
            BasicType::Long => Ok(Value::Long(self.to_i64()?)),
            BasicType::Single => Ok(Value::Single(self.to_f64()?)),
            BasicType::Double => Ok(Value::Double(self.to_f64()?)),
            BasicType::String => Ok(Value::Str(self.to_string_val()?)),
        }
    }

    /// Determine the wider numeric type for binary ops.
    /// Integer < Long < Single < Double
    pub fn common_numeric_type(a: &Value, b: &Value) -> Result<BasicType, RuntimeError> {
        if !a.is_numeric() || !b.is_numeric() {
            return Err(RuntimeError::TypeMismatch {
                msg: "type mismatch in numeric operation".into(),
            });
        }
        let rank = |v: &Value| -> u8 {
            match v {
                Value::Integer(_) => 0,
                Value::Long(_) => 1,
                Value::Single(_) => 2,
                Value::Double(_) => 3,
                Value::Str(_) => unreachable!(),
            }
        };
        let target = std::cmp::max(rank(a), rank(b));
        Ok(match target {
            0 => BasicType::Integer,
            1 => BasicType::Long,
            2 => BasicType::Single,
            _ => BasicType::Double,
        })
    }

    /// QBasic PRINT formatting:
    /// Positive numbers: " 5 " (leading space for sign, trailing space)
    /// Negative numbers: "-5 " (minus sign, trailing space)
    /// Strings: printed as-is
    pub fn format_for_print(&self) -> String {
        match self {
            Value::Integer(n) => format_number(*n as f64),
            Value::Long(n) => format_number(*n as f64),
            Value::Single(n) => format_number(*n),
            Value::Double(n) => format_number(*n),
            Value::Str(s) => s.clone(),
        }
    }

    /// WRITE# formatting: no leading/trailing spaces on numbers, strings get quoted by caller.
    pub fn format_for_write(&self) -> String {
        match self {
            Value::Integer(n) => format!("{n}"),
            Value::Long(n) => format!("{n}"),
            Value::Single(n) => format_number_raw(*n),
            Value::Double(n) => format_number_raw(*n),
            Value::Str(s) => s.clone(),
        }
    }

    pub fn is_truthy(&self) -> Result<bool, RuntimeError> {
        match self {
            Value::Integer(n) => Ok(*n != 0),
            Value::Long(n) => Ok(*n != 0),
            Value::Single(n) => Ok(*n != 0.0),
            Value::Double(n) => Ok(*n != 0.0),
            Value::Str(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot use string as boolean".into(),
            }),
        }
    }
}

fn format_number_raw(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn format_number(n: f64) -> String {
    let sign = if n >= 0.0 { " " } else { "" };
    // Check if it's an integer value
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{sign}{} ", n as i64)
    } else {
        format!("{sign}{} ", n)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(n) => write!(f, "{n}"),
            Value::Long(n) => write!(f, "{n}"),
            Value::Single(n) => write!(f, "{n}"),
            Value::Double(n) => write!(f, "{n}"),
            Value::Str(s) => write!(f, "{s}"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Str(a), Value::Str(b)) => a == b,
            (a, b) if a.is_numeric() && b.is_numeric() => {
                a.to_f64().unwrap_or(f64::NAN) == b.to_f64().unwrap_or(f64::NAN)
            }
            _ => false,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Str(a), Value::Str(b)) => a.partial_cmp(b),
            (a, b) if a.is_numeric() && b.is_numeric() => {
                let fa = a.to_f64().ok()?;
                let fb = b.to_f64().ok()?;
                fa.partial_cmp(&fb)
            }
            _ => None,
        }
    }
}
