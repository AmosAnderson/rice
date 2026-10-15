use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

use crate::error::RuntimeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasicType {
    /// Runtime-only missing value; there is no AS NULL declaration.
    Null,
    Numeric,
    Integer,
    Long,
    Single,
    Double,
    Int64,
    Bool,
    Byte,
    Bytes,
    String,
    FixedLengthString(usize),
    UserDefined(String),
}

impl BasicType {
    /// Types whose comparisons produce native boolean values.
    pub fn is_native(&self) -> bool {
        matches!(
            self,
            Self::Null
                | Self::Integer
                | Self::Long
                | Self::Single
                | Self::Int64
                | Self::Bool
                | Self::Byte
                | Self::Bytes
        )
    }

    /// Type selected by an identifier suffix, independent of case.
    pub fn from_suffix(name: &str) -> Option<Self> {
        match name.as_bytes().last()? {
            b'%' => Some(Self::Integer),
            b'&' => Some(Self::Long),
            b'!' => Some(Self::Single),
            b'#' => Some(Self::Numeric),
            b'$' => Some(Self::String),
            _ => None,
        }
    }

    /// NUMERIC and DOUBLE name the same representation.
    pub fn canonical(&self) -> Self {
        match self {
            Self::Double => Self::Numeric,
            other => other.clone(),
        }
    }

    pub fn equivalent_to(&self, other: &Self) -> bool {
        self.canonical() == other.canonical()
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Numeric(f64),
    Integer(i16),
    Long(i32),
    Single(f32),
    Int64(i64),
    Bool(bool),
    Byte(u8),
    Bytes(Vec<u8>),
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
    pub fn default_for(ty: BasicType) -> Result<Value, RuntimeError> {
        Ok(match ty {
            BasicType::Null => Value::Null,
            BasicType::Numeric | BasicType::Double => Value::Numeric(0.0),
            BasicType::Integer => Value::Integer(0),
            BasicType::Long => Value::Long(0),
            BasicType::Single => Value::Single(0.0),
            BasicType::Int64 => Value::Int64(0),
            BasicType::Bool => Value::Bool(false),
            BasicType::Byte => Value::Byte(0),
            BasicType::Bytes => Value::Bytes(Vec::new()),
            BasicType::String => Value::Str(String::new()),
            BasicType::FixedLengthString(length) => {
                return Value::Str(String::new()).coerce_to(BasicType::FixedLengthString(length));
            }
            BasicType::UserDefined(name) => {
                return Err(RuntimeError::TypeMismatch {
                    msg: format!("initializing record {name} requires its type definition"),
                });
            }
        })
    }

    pub fn get_type(&self) -> BasicType {
        match self {
            Value::Null => BasicType::Null,
            Value::Numeric(_) => BasicType::Numeric,
            Value::Integer(_) => BasicType::Integer,
            Value::Long(_) => BasicType::Long,
            Value::Single(_) => BasicType::Single,
            Value::Int64(_) => BasicType::Int64,
            Value::Bool(_) => BasicType::Bool,
            Value::Byte(_) => BasicType::Byte,
            Value::Bytes(_) => BasicType::Bytes,
            Value::Str(_) => BasicType::String,
            Value::Record { type_name, .. } => BasicType::UserDefined(type_name.clone()),
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Value::Numeric(_)
                | Value::Integer(_)
                | Value::Long(_)
                | Value::Single(_)
                | Value::Int64(_)
                | Value::Byte(_)
        )
    }

    /// Return an integer representation without converting a floating-point value.
    pub fn integer_value(&self) -> Option<i64> {
        match self {
            Value::Integer(n) => Some(i64::from(*n)),
            Value::Long(n) => Some(i64::from(*n)),
            Value::Int64(n) => Some(*n),
            Value::Byte(n) => Some(i64::from(*n)),
            _ => None,
        }
    }

    /// Floating representations widened exactly to f64, without integer conversion.
    pub fn floating_value(&self) -> Option<f64> {
        match self {
            Value::Numeric(n) => Some(*n),
            Value::Single(n) => Some(f64::from(*n)),
            _ => None,
        }
    }

    /// Convert numeric values without silently rounding an exact integer.
    pub fn to_f64(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Null => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert NULL to number".into(),
            }),
            Value::Numeric(n) => Ok(*n),
            Value::Integer(n) => Ok(f64::from(*n)),
            Value::Long(n) => Ok(f64::from(*n)),
            Value::Single(n) => Ok(f64::from(*n)),
            Value::Int64(n) => {
                let float = *n as f64;
                if compare_integer_float(*n, float) != Some(Ordering::Equal) {
                    return Err(RuntimeError::Overflow {
                        msg: "integer cannot be represented exactly as a floating-point number"
                            .into(),
                    });
                }
                Ok(float)
            }
            Value::Byte(n) => Ok(f64::from(*n)),
            Value::Bool(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert boolean to number implicitly".into(),
            }),
            Value::Bytes(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert bytes to number".into(),
            }),
            Value::Str(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert string to number".into(),
            }),
            Value::Record { .. } => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert record to number".into(),
            }),
        }
    }

    /// Explicit numeric conversion that permits rounding a 64-bit integer.
    pub fn to_f64_lossy(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Int64(n) => Ok(*n as f64),
            _ => self.to_f64(),
        }
    }

    pub fn to_i64(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Null => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert NULL to integer".into(),
            }),
            Value::Integer(n) => Ok(i64::from(*n)),
            Value::Long(n) => Ok(i64::from(*n)),
            Value::Single(n) => Value::Numeric(f64::from(*n)).to_i64(),
            Value::Int64(n) => Ok(*n),
            Value::Byte(n) => Ok(i64::from(*n)),
            Value::Numeric(n) => {
                // i64::MAX rounds up to 2^63 as f64, so the upper bound is exclusive.
                if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(n) {
                    return Err(RuntimeError::Overflow {
                        msg: "number is outside the integer range".into(),
                    });
                }
                Ok(*n as i64)
            }
            Value::Bool(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert boolean to integer implicitly".into(),
            }),
            Value::Bytes(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert bytes to integer".into(),
            }),
            Value::Str(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert string to integer".into(),
            }),
            Value::Record { .. } => Err(RuntimeError::TypeMismatch {
                msg: "cannot convert record to integer".into(),
            }),
        }
    }

    /// Reject fractional floating-point values when an exact integer is required.
    pub fn to_exact_i64(&self) -> Result<i64, RuntimeError> {
        let integer = self.to_i64()?;
        if let Some(n) = self.floating_value()
            && n.fract() != 0.0
        {
            return Err(RuntimeError::TypeMismatch {
                msg: "expected an exact integer, got a fractional number".into(),
            });
        }
        Ok(integer)
    }

    /// Convert to single precision with rounding, rejecting finite overflow.
    pub fn to_f32(&self) -> Result<f32, RuntimeError> {
        let number = match self {
            Value::Int64(n) => return Ok(*n as f32),
            Value::Single(n) => return Ok(*n),
            _ => self.to_f64()?,
        };
        let single = number as f32;
        if number.is_finite() && !single.is_finite() {
            return Err(RuntimeError::Overflow {
                msg: "number is outside the SINGLE range".into(),
            });
        }
        Ok(single)
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
            BasicType::Null => match self {
                Value::Null => Ok(Value::Null),
                _ => Err(RuntimeError::TypeMismatch {
                    msg: "expected NULL".into(),
                }),
            },
            BasicType::Numeric | BasicType::Double => Ok(Value::Numeric(self.to_f64()?)),
            BasicType::Integer => {
                let number =
                    i16::try_from(self.to_exact_i64()?).map_err(|_| RuntimeError::Overflow {
                        msg: "number is outside the INTEGER range -32768 through 32767".into(),
                    })?;
                Ok(Value::Integer(number))
            }
            BasicType::Long => {
                let number =
                    i32::try_from(self.to_exact_i64()?).map_err(|_| RuntimeError::Overflow {
                        msg: "number is outside the LONG range -2147483648 through 2147483647"
                            .into(),
                    })?;
                Ok(Value::Long(number))
            }
            BasicType::Single => Ok(Value::Single(self.to_f32()?)),
            BasicType::Int64 => Ok(Value::Int64(self.to_exact_i64()?)),
            BasicType::Byte => {
                let n = self.to_exact_i64()?;
                let byte = u8::try_from(n).map_err(|_| RuntimeError::Overflow {
                    msg: "number is outside the BYTE range 0 through 255".into(),
                })?;
                Ok(Value::Byte(byte))
            }
            BasicType::Bool => match self {
                Value::Bool(b) => Ok(Value::Bool(*b)),
                Value::Integer(n) => Ok(Value::Bool(*n != 0)),
                Value::Long(n) => Ok(Value::Bool(*n != 0)),
                Value::Single(n) if n.is_finite() => Ok(Value::Bool(*n != 0.0)),
                Value::Int64(n) => Ok(Value::Bool(*n != 0)),
                Value::Byte(n) => Ok(Value::Bool(*n != 0)),
                Value::Numeric(n) if n.is_finite() => Ok(Value::Bool(*n != 0.0)),
                _ => Err(RuntimeError::TypeMismatch {
                    msg: "BOOL requires a boolean or finite number".into(),
                }),
            },
            BasicType::Bytes => match self {
                Value::Bytes(bytes) => Ok(Value::Bytes(bytes.clone())),
                _ => Err(RuntimeError::TypeMismatch {
                    msg: "BYTES requires a byte buffer".into(),
                }),
            },
            BasicType::String => Ok(Value::Str(self.to_string_val()?)),
            BasicType::FixedLengthString(length) => {
                let source = self.to_string_val()?;
                let mut result: String = source.chars().take(length).collect();
                let padding = length - result.chars().count();
                result
                    .try_reserve(padding)
                    .map_err(|_| RuntimeError::Overflow {
                        msg: "fixed-length string exceeds available memory".into(),
                    })?;
                result.extend(std::iter::repeat_n(' ', padding));
                Ok(Value::Str(result))
            }
            BasicType::UserDefined(expected) => match self {
                Value::Record { type_name, .. } if type_name.eq_ignore_ascii_case(&expected) => {
                    Ok(self.clone())
                }
                _ => Err(RuntimeError::TypeMismatch {
                    msg: format!("expected a record of type {expected}"),
                }),
            },
        }
    }

    /// Rice PRINT formatting:
    /// Numbers print without leading/trailing spaces.
    /// Strings: printed as-is.
    pub fn format_for_print(&self) -> String {
        match self {
            Value::Null => "NULL".into(),
            Value::Numeric(n) => format_number(*n),
            Value::Integer(n) => n.to_string(),
            Value::Long(n) => n.to_string(),
            Value::Single(n) => n.to_string(),
            Value::Int64(n) => n.to_string(),
            Value::Bool(b) => format_bool(*b).into(),
            Value::Byte(n) => n.to_string(),
            Value::Bytes(bytes) => format_bytes(bytes),
            Value::Str(s) => s.clone(),
            Value::Record { type_name, .. } => format!("[{type_name}]"),
        }
    }

    /// WRITE# formatting: no leading/trailing spaces on numbers, strings get quoted by caller.
    pub fn format_for_write(&self) -> String {
        self.format_for_print()
    }

    pub fn is_truthy(&self) -> Result<bool, RuntimeError> {
        match self {
            Value::Null => Err(RuntimeError::TypeMismatch {
                msg: "cannot use NULL as boolean".into(),
            }),
            Value::Numeric(n) => Ok(*n != 0.0),
            Value::Integer(n) => Ok(*n != 0),
            Value::Long(n) => Ok(*n != 0),
            Value::Single(n) => Ok(*n != 0.0),
            Value::Int64(n) => Ok(*n != 0),
            Value::Bool(b) => Ok(*b),
            Value::Byte(n) => Ok(*n != 0),
            Value::Bytes(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot use bytes as boolean".into(),
            }),
            Value::Str(_) => Err(RuntimeError::TypeMismatch {
                msg: "cannot use string as boolean".into(),
            }),
            Value::Record { .. } => Err(RuntimeError::TypeMismatch {
                msg: "cannot use record as boolean".into(),
            }),
        }
    }
}

/// Compare without first rounding the integer to f64. The exclusive upper bound
/// also prevents Rust's saturating float cast from equating i64::MAX with 2^63.
fn compare_integer_float(integer: i64, number: f64) -> Option<Ordering> {
    if number.is_nan() {
        return None;
    }
    if number < -9_223_372_036_854_775_808.0 {
        return Some(Ordering::Greater);
    }
    if number >= 9_223_372_036_854_775_808.0 {
        return Some(Ordering::Less);
    }
    match integer.cmp(&(number as i64)) {
        Ordering::Equal => 0.0_f64.partial_cmp(&number.fract()),
        ordering => Some(ordering),
    }
}

fn compare_numeric_values(left: &Value, right: &Value) -> Option<Ordering> {
    match (left.integer_value(), right.integer_value()) {
        (Some(a), Some(b)) => Some(a.cmp(&b)),
        (Some(a), None) => right
            .floating_value()
            .and_then(|b| compare_integer_float(a, b)),
        (None, Some(b)) => left
            .floating_value()
            .and_then(|a| compare_integer_float(b, a))
            .map(Ordering::reverse),
        (None, None) => left.floating_value()?.partial_cmp(&right.floating_value()?),
    }
}

fn format_bool(value: bool) -> &'static str {
    if value { "TRUE" } else { "FALSE" }
}

fn format_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::from("&H");
    for byte in bytes {
        // Writing into a String cannot fail.
        write!(result, "{byte:02X}").expect("writing to String is infallible");
    }
    result
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
            Value::Null => f.write_str("NULL"),
            Value::Numeric(n) => write!(f, "{n}"),
            Value::Integer(n) => write!(f, "{n}"),
            Value::Long(n) => write!(f, "{n}"),
            Value::Single(n) => write!(f, "{n}"),
            Value::Int64(n) => write!(f, "{n}"),
            Value::Bool(b) => f.write_str(format_bool(*b)),
            Value::Byte(n) => write!(f, "{n}"),
            Value::Bytes(bytes) => f.write_str(&format_bytes(bytes)),
            Value::Str(s) => write!(f, "{s}"),
            Value::Record { type_name, .. } => write!(f, "[{type_name}]"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
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
            _ => compare_numeric_values(self, other) == Some(Ordering::Equal),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            // Keep Rust equality/order consistency; Rice rejects NULL ordering
            // expressions explicitly before reaching this comparison.
            (Value::Null, Value::Null) => Some(Ordering::Equal),
            (Value::Str(a), Value::Str(b)) => a.partial_cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
            (Value::Bytes(a), Value::Bytes(b)) => a.partial_cmp(b),
            _ => compare_numeric_values(self, other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_is_distinct_from_empty_values_and_preserves_equality_order_consistency() {
        assert_eq!(Value::Null.get_type(), BasicType::Null);
        assert!(BasicType::Null.is_native());
        assert!(!Value::Null.is_numeric());
        assert_eq!(Value::default_for(BasicType::Null).unwrap(), Value::Null);
        assert_eq!(Value::Null.coerce_to(BasicType::Null).unwrap(), Value::Null);
        assert_eq!(Value::Null, Value::Null);
        assert_eq!(Value::Null.partial_cmp(&Value::Null), Some(Ordering::Equal));
        assert_eq!(Value::Null.to_string(), "NULL");
        assert_eq!(Value::Null.format_for_print(), "NULL");
        assert_eq!(Value::Null.format_for_write(), "NULL");
        for empty in [
            Value::Numeric(0.0),
            Value::Int64(0),
            Value::Bool(false),
            Value::Str(String::new()),
            Value::Bytes(Vec::new()),
        ] {
            assert_ne!(Value::Null, empty);
            assert_eq!(Value::Null.partial_cmp(&empty), None);
            assert_eq!(empty.partial_cmp(&Value::Null), None);
            assert!(empty.coerce_to(BasicType::Null).is_err());
        }
    }

    #[test]
    fn null_rejects_typed_assignment_conversion_and_truthiness() {
        for ty in [
            BasicType::Numeric,
            BasicType::Double,
            BasicType::Integer,
            BasicType::Long,
            BasicType::Single,
            BasicType::Int64,
            BasicType::Bool,
            BasicType::Byte,
            BasicType::Bytes,
            BasicType::String,
            BasicType::FixedLengthString(0),
            BasicType::UserDefined("POINT".into()),
        ] {
            assert!(matches!(
                Value::Null.coerce_to(ty),
                Err(RuntimeError::TypeMismatch { .. })
            ));
        }
        assert!(Value::Null.to_f64().is_err());
        assert!(Value::Null.to_f64_lossy().is_err());
        assert!(Value::Null.to_f32().is_err());
        assert!(Value::Null.to_i64().is_err());
        assert!(Value::Null.to_exact_i64().is_err());
        assert!(Value::Null.to_string_val().is_err());
        assert!(Value::Null.is_truthy().is_err());
    }

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

    #[test]
    fn declared_types_have_distinct_defaults_and_native_comparisons() {
        for (ty, expected) in [
            (BasicType::Integer, Value::Integer(0)),
            (BasicType::Long, Value::Long(0)),
            (BasicType::Single, Value::Single(0.0)),
            (BasicType::Int64, Value::Int64(0)),
            (BasicType::Bool, Value::Bool(false)),
            (BasicType::Byte, Value::Byte(0)),
            (BasicType::Bytes, Value::Bytes(Vec::new())),
        ] {
            assert!(ty.is_native());
            let default = Value::default_for(ty.clone()).unwrap();
            assert_eq!(default, expected);
            assert_eq!(default.get_type(), ty);
        }
        for ty in [BasicType::Numeric, BasicType::Double] {
            assert!(!ty.is_native());
            assert!(matches!(
                Value::default_for(ty).unwrap(),
                Value::Numeric(0.0)
            ));
        }
        assert!(!BasicType::String.is_native());
        assert!(!BasicType::FixedLengthString(10).is_native());
        assert!(!BasicType::UserDefined("RECORD".into()).is_native());
        assert_eq!(
            Value::default_for(BasicType::FixedLengthString(3)).unwrap(),
            Value::Str("   ".into())
        );
    }

    #[test]
    fn unavailable_default_storage_and_record_schema_report_errors() {
        assert!(matches!(
            Value::default_for(BasicType::FixedLengthString(usize::MAX)),
            Err(RuntimeError::Overflow { .. })
        ));
        assert!(matches!(
            Value::Str("é".into()).coerce_to(BasicType::FixedLengthString(usize::MAX)),
            Err(RuntimeError::Overflow { .. })
        ));
        assert!(matches!(
            Value::default_for(BasicType::UserDefined("POINT".into())),
            Err(RuntimeError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn suffix_types_and_double_alias_are_canonical() {
        for (name, ty) in [
            ("COUNT%", BasicType::Integer),
            ("COUNT&", BasicType::Long),
            ("VALUE!", BasicType::Single),
            ("VALUE#", BasicType::Numeric),
            ("TEXT$", BasicType::String),
        ] {
            assert_eq!(BasicType::from_suffix(name), Some(ty));
        }
        assert_eq!(BasicType::from_suffix("name"), None);
        assert_eq!(BasicType::from_suffix(""), None);
        assert!(BasicType::Numeric.equivalent_to(&BasicType::Double));
        assert!(BasicType::Double.equivalent_to(&BasicType::Numeric));
        assert!(!BasicType::Single.equivalent_to(&BasicType::Numeric));
    }

    #[test]
    fn integer_and_long_assignments_require_exact_in_range_numbers() {
        for (ty, minimum, maximum) in [
            (BasicType::Integer, i64::from(i16::MIN), i64::from(i16::MAX)),
            (BasicType::Long, i64::from(i32::MIN), i64::from(i32::MAX)),
        ] {
            for number in [minimum, -1, 0, 1, maximum] {
                let value = Value::Int64(number).coerce_to(ty.clone()).unwrap();
                assert_eq!(value.get_type(), ty);
                assert_eq!(value.integer_value(), Some(number));
                assert_eq!(value.to_f64().unwrap(), number as f64);
                assert_eq!(value.is_truthy().unwrap(), number != 0);
            }
            for number in [minimum - 1, maximum + 1] {
                assert!(matches!(
                    Value::Int64(number).coerce_to(ty.clone()),
                    Err(RuntimeError::Overflow { .. })
                ));
            }
            for invalid in [
                Value::Numeric(1.5),
                Value::Single(-1.5),
                Value::Str("1".into()),
                Value::Bool(true),
            ] {
                assert!(matches!(
                    invalid.coerce_to(ty.clone()),
                    Err(RuntimeError::TypeMismatch { .. })
                ));
            }
        }
    }

    #[test]
    fn single_storage_rounds_checks_overflow_and_preserves_float_special_values() {
        let rounded = Value::Numeric(0.1).coerce_to(BasicType::Single).unwrap();
        assert!(matches!(rounded, Value::Single(n) if n == 0.1_f32));
        assert_eq!(rounded.to_f64().unwrap(), f64::from(0.1_f32));
        assert_eq!(rounded.format_for_print(), "0.1");
        assert_eq!(rounded.floating_value(), Some(f64::from(0.1_f32)));
        assert_eq!(Value::Single(1.5).to_i64().unwrap(), 1);
        assert!(matches!(
            Value::Single(1.5).to_exact_i64(),
            Err(RuntimeError::TypeMismatch { .. })
        ));
        assert_eq!(Value::Int64(i64::MAX).to_f32().unwrap(), i64::MAX as f32);
        for number in [f64::MAX, -f64::MAX] {
            assert!(matches!(
                Value::Numeric(number).coerce_to(BasicType::Single),
                Err(RuntimeError::Overflow { .. })
            ));
        }
        assert!(
            matches!(Value::Numeric(f64::NAN).coerce_to(BasicType::Single).unwrap(), Value::Single(n) if n.is_nan())
        );
        assert!(
            matches!(Value::Numeric(f64::INFINITY).coerce_to(BasicType::Single).unwrap(), Value::Single(n) if n.is_infinite())
        );
        assert_eq!(Value::Single(f32::MIN_POSITIVE).is_truthy().unwrap(), true);
        assert_eq!(Value::Single(-0.0).is_truthy().unwrap(), false);
    }

    #[test]
    fn string_and_record_assignments_enforce_types_and_fixed_character_lengths() {
        for (source, length, expected) in [
            ("é猫", 4, "é猫  "),
            ("é猫🦀end", 3, "é猫🦀"),
            ("abc", 0, ""),
            ("", 2, "  "),
        ] {
            assert_eq!(
                Value::Str(source.into())
                    .coerce_to(BasicType::FixedLengthString(length))
                    .unwrap(),
                Value::Str(expected.into())
            );
        }
        for invalid in [
            Value::Numeric(1.0),
            Value::Integer(1),
            Value::Bool(true),
            Value::Bytes(vec![65]),
        ] {
            assert!(invalid.coerce_to(BasicType::String).is_err());
            assert!(invalid.coerce_to(BasicType::FixedLengthString(4)).is_err());
        }
        let record = Value::Record {
            type_name: "POINT".into(),
            fields: HashMap::from([("X".into(), Value::Integer(2))]),
        };
        assert_eq!(
            record
                .coerce_to(BasicType::UserDefined("POINT".into()))
                .unwrap(),
            record
        );
        assert!(
            record
                .coerce_to(BasicType::UserDefined("OTHER".into()))
                .is_err()
        );
        assert!(
            Value::Numeric(0.0)
                .coerce_to(BasicType::UserDefined("POINT".into()))
                .is_err()
        );
    }

    #[test]
    fn numeric_ordering_crosses_integer_long_single_without_precision_loss() {
        assert!(Value::Long(16_777_217) > Value::Single(16_777_216.0));
        assert!(Value::Single(16_777_216.0) < Value::Long(16_777_217));
        assert_eq!(Value::Integer(-32768), Value::Single(-32768.0));
        assert_eq!(Value::Long(i32::MAX), Value::Numeric(i32::MAX as f64));
        assert_eq!(Value::Single(0.1), Value::Numeric(f64::from(0.1_f32)));
        assert_ne!(Value::Single(0.1), Value::Numeric(0.1));
        assert_eq!(
            Value::Single(f32::NAN).partial_cmp(&Value::Integer(0)),
            None
        );
        assert_eq!(
            Value::Integer(0).partial_cmp(&Value::Single(f32::NAN)),
            None
        );
    }

    #[test]
    fn int64_conversion_preserves_exact_values_and_rejects_fractional_input() {
        for n in [
            i64::MIN,
            -9_007_199_254_740_993,
            0,
            9_007_199_254_740_993,
            i64::MAX,
        ] {
            let value = Value::Int64(n);
            assert_eq!(value.integer_value(), Some(n));
            assert_eq!(value.to_i64().unwrap(), n);
            assert_eq!(value.to_exact_i64().unwrap(), n);
            assert_eq!(value.coerce_to(BasicType::Int64).unwrap(), value);
        }
        for n in [-1.9, -0.5, 0.5, 1.9, f64::from_bits(1)] {
            assert!(matches!(
                Value::Numeric(n).to_exact_i64(),
                Err(RuntimeError::TypeMismatch { .. })
            ));
        }
        for n in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            9_223_372_036_854_775_808.0,
        ] {
            assert!(matches!(
                Value::Numeric(n).coerce_to(BasicType::Int64),
                Err(RuntimeError::Overflow { .. })
            ));
        }
        assert_eq!(Value::Numeric(-0.0).to_exact_i64().unwrap(), 0);
        assert_eq!(
            Value::Numeric(i64::MIN as f64).to_exact_i64().unwrap(),
            i64::MIN
        );
        assert_eq!(
            Value::Numeric(42.0).coerce_to(BasicType::Int64).unwrap(),
            Value::Int64(42)
        );
        assert!(Value::Str("42".into()).coerce_to(BasicType::Int64).is_err());
        assert_eq!(Value::Numeric(42.0).integer_value(), None);
    }

    #[test]
    fn implicit_float_conversion_never_rounds_exact_integers() {
        for n in [
            i64::MIN,
            -9_007_199_254_740_992,
            0,
            9_007_199_254_740_992,
            9_007_199_254_740_994,
            9_223_372_036_854_774_784,
        ] {
            assert_eq!(Value::Int64(n).to_f64().unwrap(), n as f64);
            assert_eq!(
                Value::Int64(n).coerce_to(BasicType::Double).unwrap(),
                Value::Numeric(n as f64)
            );
        }
        for n in [
            i64::MIN + 1,
            -9_007_199_254_740_993,
            9_007_199_254_740_993,
            i64::MAX,
        ] {
            assert!(matches!(
                Value::Int64(n).to_f64(),
                Err(RuntimeError::Overflow { .. })
            ));
            assert!(Value::Int64(n).coerce_to(BasicType::Numeric).is_err());
            assert_eq!(Value::Int64(n).to_f64_lossy().unwrap(), n as f64);
        }
        assert!(Value::Numeric(f64::NAN).to_f64().unwrap().is_nan());
        assert_eq!(
            Value::Numeric(f64::INFINITY).to_f64().unwrap(),
            f64::INFINITY
        );
    }

    #[test]
    fn byte_conversion_checks_integrality_and_range() {
        for n in [0_u8, 1, 127, 255] {
            let value = Value::Byte(n);
            assert!(value.is_numeric());
            assert_eq!(value.integer_value(), Some(i64::from(n)));
            assert_eq!(value.to_i64().unwrap(), i64::from(n));
            assert_eq!(value.to_f64().unwrap(), f64::from(n));
            assert_eq!(
                Value::Numeric(f64::from(n))
                    .coerce_to(BasicType::Byte)
                    .unwrap(),
                value
            );
            assert_eq!(
                Value::Int64(i64::from(n))
                    .coerce_to(BasicType::Byte)
                    .unwrap(),
                value
            );
            assert_eq!(
                value.coerce_to(BasicType::Int64).unwrap(),
                Value::Int64(i64::from(n))
            );
        }
        for n in [-1, 256, i64::MIN, i64::MAX] {
            assert!(matches!(
                Value::Int64(n).coerce_to(BasicType::Byte),
                Err(RuntimeError::Overflow { .. })
            ));
        }
        for value in [
            Value::Numeric(1.5),
            Value::Bool(true),
            Value::Str("1".into()),
        ] {
            assert!(matches!(
                value.coerce_to(BasicType::Byte),
                Err(RuntimeError::TypeMismatch { .. })
            ));
        }
    }

    #[test]
    fn bool_conversion_is_explicit_and_requires_finite_numbers() {
        for (value, expected) in [
            (Value::Numeric(0.0), false),
            (Value::Numeric(-0.0), false),
            (Value::Numeric(-0.25), true),
            (Value::Numeric(f64::from_bits(1)), true),
            (Value::Int64(i64::MAX), true),
            (Value::Int64(0), false),
            (Value::Byte(255), true),
            (Value::Byte(0), false),
            (Value::Bool(false), false),
            (Value::Bool(true), true),
        ] {
            assert_eq!(
                value.coerce_to(BasicType::Bool).unwrap(),
                Value::Bool(expected)
            );
        }
        for value in [
            Value::Numeric(f64::NAN),
            Value::Numeric(f64::INFINITY),
            Value::Numeric(f64::NEG_INFINITY),
            Value::Str("TRUE".into()),
            Value::Bytes(vec![1]),
        ] {
            assert!(matches!(
                value.coerce_to(BasicType::Bool),
                Err(RuntimeError::TypeMismatch { .. })
            ));
        }
        for b in [false, true] {
            let value = Value::Bool(b);
            assert!(!value.is_numeric());
            assert_eq!(value.integer_value(), None);
            assert_eq!(value.is_truthy().unwrap(), b);
            assert!(value.to_f64().is_err());
            assert!(value.to_i64().is_err());
            assert!(value.to_exact_i64().is_err());
        }
    }

    #[test]
    fn bytes_preserve_all_byte_values_and_remain_distinct_from_strings() {
        let bytes: Vec<u8> = (0..=255).collect();
        let value = Value::Bytes(bytes.clone());
        assert_eq!(value.coerce_to(BasicType::Bytes).unwrap(), value);
        assert_eq!(value.get_type(), BasicType::Bytes);
        assert!(!value.is_numeric());
        assert!(value.to_f64().is_err());
        assert!(value.to_i64().is_err());
        assert!(value.to_string_val().is_err());
        assert!(value.is_truthy().is_err());
        assert!(
            Value::Str(bytes_to_basic_string(&bytes))
                .coerce_to(BasicType::Bytes)
                .is_err()
        );
        assert_ne!(Value::Bytes(Vec::new()), Value::Str(String::new()));
        assert_ne!(Value::Bytes(vec![0]), Value::Numeric(0.0));
    }

    #[test]
    fn mixed_numeric_comparisons_are_exact_at_float_and_integer_boundaries() {
        for (integer, number, expected) in [
            (i64::MAX, 9_223_372_036_854_775_808.0, Ordering::Less),
            (i64::MAX, 9_223_372_036_854_774_784.0, Ordering::Greater),
            (i64::MIN, -9_223_372_036_854_775_808.0, Ordering::Equal),
            (
                i64::MIN + 1,
                -9_223_372_036_854_775_808.0,
                Ordering::Greater,
            ),
            (i64::MIN, -9_223_372_036_854_777_856.0, Ordering::Greater),
            (
                9_007_199_254_740_993,
                9_007_199_254_740_992.0,
                Ordering::Greater,
            ),
            (
                -9_007_199_254_740_993,
                -9_007_199_254_740_992.0,
                Ordering::Less,
            ),
            (1, 1.5, Ordering::Less),
            (-1, -1.5, Ordering::Greater),
            (0, 0.5, Ordering::Less),
            (0, -0.5, Ordering::Greater),
            (0, -0.0, Ordering::Equal),
            (0, f64::from_bits(1), Ordering::Less),
            (0, -f64::from_bits(1), Ordering::Greater),
            (i64::MAX, f64::INFINITY, Ordering::Less),
            (i64::MIN, f64::NEG_INFINITY, Ordering::Greater),
        ] {
            let left = Value::Int64(integer);
            let right = Value::Numeric(number);
            assert_eq!(
                left.partial_cmp(&right),
                Some(expected),
                "{integer} vs {number}"
            );
            assert_eq!(
                right.partial_cmp(&left),
                Some(expected.reverse()),
                "{number} vs {integer}"
            );
            assert_eq!(left == right, expected == Ordering::Equal);
            assert_eq!(right == left, expected == Ordering::Equal);
        }
        for value in [
            Value::Int64(i64::MIN),
            Value::Int64(0),
            Value::Int64(i64::MAX),
            Value::Byte(1),
            Value::Numeric(0.0),
        ] {
            let nan = Value::Numeric(f64::NAN);
            assert_eq!(value.partial_cmp(&nan), None);
            assert_eq!(nan.partial_cmp(&value), None);
            assert_ne!(value, nan);
        }
        assert!(Value::Int64(i64::MIN) < Value::Int64(i64::MAX));
        assert_eq!(Value::Byte(255), Value::Int64(255));
        assert_eq!(Value::Byte(255), Value::Numeric(255.0));
        assert!(Value::Byte(255) < Value::Int64(256));
        assert!(Value::Byte(255) < Value::Numeric(255.5));
    }

    #[test]
    fn bool_and_bytes_comparisons_do_not_coerce_other_types() {
        assert!(Value::Bool(false) < Value::Bool(true));
        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert!(Value::Bytes(vec![0, 1]) < Value::Bytes(vec![0, 2]));
        assert!(Value::Bytes(vec![0]) < Value::Bytes(vec![0, 0]));
        assert_eq!(Value::Bytes(vec![0, 255]), Value::Bytes(vec![0, 255]));
        for numeric in [Value::Numeric(1.0), Value::Int64(1), Value::Byte(1)] {
            assert_ne!(Value::Bool(true), numeric);
            assert_eq!(Value::Bool(true).partial_cmp(&numeric), None);
            assert_eq!(numeric.partial_cmp(&Value::Bool(true)), None);
        }
        assert_eq!(
            Value::Bytes(vec![65]).partial_cmp(&Value::Str("A".into())),
            None
        );
    }

    #[test]
    fn new_value_formatting_is_exact_and_unambiguous() {
        for (value, expected) in [
            (Value::Int64(i64::MIN), "-9223372036854775808"),
            (Value::Int64(i64::MAX), "9223372036854775807"),
            (Value::Bool(false), "FALSE"),
            (Value::Bool(true), "TRUE"),
            (Value::Byte(255), "255"),
            (Value::Bytes(Vec::new()), "&H"),
            (Value::Bytes(vec![0, 1, 15, 127, 255]), "&H00010F7FFF"),
        ] {
            assert_eq!(value.format_for_print(), expected);
            assert_eq!(value.format_for_write(), expected);
            assert_eq!(value.to_string(), expected);
        }
    }
}
