use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;

use crate::error::RuntimeError;
use crate::value::{BasicType, Value, basic_string_to_bytes, bytes_to_basic_string};

pub type BuiltinFn = fn(&[Value]) -> Result<Value, RuntimeError>;

#[derive(Clone, Copy)]
enum BuiltinArity {
    Exact(usize),
    Variadic,
}

pub struct BuiltinRegistry {
    functions: HashMap<String, (BuiltinFn, BuiltinArity)>,
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            functions: HashMap::new(),
        };

        // Math
        reg.register("ABS", builtin_abs, 1);
        reg.register("INT", builtin_int, 1);
        reg.register("FIX", builtin_fix, 1);
        reg.register("SGN", builtin_sgn, 1);
        reg.register("SQR", builtin_sqr, 1);
        reg.register("SIN", builtin_sin, 1);
        reg.register("COS", builtin_cos, 1);
        reg.register("TAN", builtin_tan, 1);
        reg.register("ATN", builtin_atn, 1);
        reg.register("EXP", builtin_exp, 1);
        reg.register("LOG", builtin_log, 1);
        reg.register_variadic("ROUND", builtin_round);
        // Additional mathematical functions
        reg.register("ASIN", builtin_asin, 1);
        reg.register("ACOS", builtin_acos, 1);
        reg.register("COT", builtin_cot, 1);
        reg.register("CSC", builtin_csc, 1);
        reg.register("SEC", builtin_sec, 1);
        reg.register("ANGLE", builtin_angle, 2);
        reg.register("CEIL", builtin_ceil, 1);
        reg.register("CINT", builtin_cint, 1);
        reg.register("CLNG", builtin_clng, 1);
        reg.register("CSNG", builtin_csng, 1);
        reg.register("CDBL", builtin_cdbl, 1);
        reg.register("CINT64", builtin_cint64, 1);
        reg.register("CBOOL", builtin_cbool, 1);
        reg.register("CBYTE", builtin_cbyte, 1);
        reg.register("TRUE", builtin_true, 0);
        reg.register("FALSE", builtin_false, 0);
        reg.register("NULL", builtin_null, 0);
        reg.register("ISNULL", builtin_isnull, 1);
        reg.register("TYPEOF$", builtin_typeof, 1);
        reg.register_variadic("TRUNCATE", builtin_truncate);
        reg.register("REMAINDER", builtin_remainder, 2);
        reg.register_variadic("MIN", builtin_min);
        reg.register_variadic("MAX", builtin_max);
        reg.register("MAXNUM", builtin_maxnum, 0);
        reg.register("PI", builtin_pi, 0);
        // RND is handled as a stateful function in the interpreter

        // String
        reg.register("LEN", builtin_len, 1);
        reg.register_variadic("INSTR", builtin_instr);
        reg.register_variadic("INSTRREV", builtin_instrrev);
        reg.register_variadic("STRCOMP", builtin_strcomp);
        reg.register_variadic("REPLACE$", builtin_replace);
        reg.register("LTRIM$", builtin_ltrim, 1);
        reg.register("RTRIM$", builtin_rtrim, 1);
        reg.register("SPACE$", builtin_space, 1);
        reg.register("STRING$", builtin_string_fn, 2);
        reg.register("CHR$", builtin_chr, 1);
        reg.register("ASC", builtin_asc, 1);
        reg.register("STR$", builtin_str, 1);
        reg.register("VAL", builtin_val, 1);
        reg.register("LEFT$", builtin_left, 2);
        reg.register("RIGHT$", builtin_right, 2);
        reg.register_variadic("MID$", builtin_mid);
        reg.register("UCASE$", builtin_ucase, 1);
        reg.register("LCASE$", builtin_lcase, 1);
        reg.register("HEX$", builtin_hex, 1);
        reg.register("OCT$", builtin_oct, 1);
        reg.register("MKI$", builtin_mki, 1);
        reg.register("MKL$", builtin_mkl, 1);
        reg.register("MKS$", builtin_mks, 1);
        reg.register("MKD$", builtin_mkd, 1);
        reg.register("CVI", builtin_cvi, 1);
        reg.register("CVL", builtin_cvl, 1);
        reg.register("CVS", builtin_cvs, 1);
        reg.register("CVD", builtin_cvd, 1);

        // Binary buffers are distinct from Unicode strings.
        reg.register_variadic("BYTES", builtin_bytes);
        reg.register_variadic("BYTEBUFFER", builtin_bytebuffer);
        reg.register("BYTELEN", builtin_bytelen, 1);
        reg.register("BYTEAT", builtin_byteat, 2);
        reg.register("BYTESET", builtin_byteset, 3);
        reg.register_variadic("BYTESLICE", builtin_byteslice);
        reg.register("UTF8ENCODE", builtin_utf8encode, 1);
        reg.register("UTF8DECODE$", builtin_utf8decode, 1);
        reg.register("HEXENCODE$", builtin_hexencode, 1);
        reg.register("HEXDECODE", builtin_hexdecode, 1);

        // Misc
        reg.register("TIMER", builtin_timer, 0);
        reg.register("DATE$", builtin_date, 0);
        reg.register("TIME$", builtin_time, 0);
        reg.register("ENVIRON$", builtin_environ, 1);
        reg.register("CURDIR$", builtin_curdir, 0);
        reg.register("COMMAND$", builtin_command, 0);

        reg
    }
    fn register(&mut self, name: &str, func: BuiltinFn, args: usize) {
        self.functions
            .insert(name.to_string(), (func, BuiltinArity::Exact(args)));
    }

    fn register_variadic(&mut self, name: &str, func: BuiltinFn) {
        self.functions
            .insert(name.to_string(), (func, BuiltinArity::Variadic));
    }

    pub fn call(&self, name: &str, args: &[Value]) -> Result<Option<Value>, RuntimeError> {
        let name_upper = name.to_uppercase();
        // Try name as-is, then with $
        let func_info = self
            .functions
            .get(&name_upper)
            .or_else(|| self.functions.get(&format!("{}$", name_upper)));
        if let Some((func, arity)) = func_info {
            if let BuiltinArity::Exact(expected) = arity
                && args.len() != *expected
            {
                return Err(RuntimeError::ArityMismatch {
                    expected: *expected,
                    got: args.len(),
                });
            }
            return Ok(Some(func(args)?));
        }
        Ok(None)
    }

    pub fn exists(&self, name: &str) -> bool {
        let upper = name.to_uppercase();
        self.functions.contains_key(&upper) || self.functions.contains_key(&format!("{}$", upper))
    }
}

// Explicit conversions and native binary buffers.

fn builtin_cint64(args: &[Value]) -> Result<Value, RuntimeError> {
    let number = match &args[0] {
        Value::Str(text) => text.trim().parse::<i64>().map_err(|error| {
            if matches!(
                error.kind(),
                std::num::IntErrorKind::PosOverflow | std::num::IntErrorKind::NegOverflow
            ) {
                RuntimeError::Overflow {
                    msg: "CINT64: decimal integer is outside the signed 64-bit range".into(),
                }
            } else {
                RuntimeError::IllegalFunctionCall {
                    msg: "CINT64 expects a decimal integer".into(),
                }
            }
        })?,
        Value::Bool(value) => i64::from(*value),
        value => value.to_exact_i64()?,
    };
    Ok(Value::Int64(number))
}

fn builtin_cbool(args: &[Value]) -> Result<Value, RuntimeError> {
    let value = match &args[0] {
        Value::Bool(value) => *value,
        Value::Integer(value) => *value != 0,
        Value::Long(value) => *value != 0,
        Value::Int64(value) => *value != 0,
        Value::Byte(value) => *value != 0,
        Value::Single(value) if value.is_finite() => *value != 0.0,
        Value::Numeric(value) if value.is_finite() => *value != 0.0,
        Value::Numeric(_) | Value::Single(_) => {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: "CBOOL expects a finite number".into(),
            });
        }
        _ => {
            return Err(RuntimeError::TypeMismatch {
                msg: "CBOOL expects a boolean or number".into(),
            });
        }
    };
    Ok(Value::Bool(value))
}

fn exact_byte(value: &Value) -> Result<u8, RuntimeError> {
    let number = match value {
        Value::Bool(value) => i64::from(*value),
        value => value.to_exact_i64()?,
    };
    u8::try_from(number).map_err(|_| RuntimeError::Overflow {
        msg: "byte value must be in the range 0..255".into(),
    })
}

fn builtin_cbyte(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Byte(exact_byte(&args[0])?))
}

fn builtin_true(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(true))
}

fn builtin_false(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(false))
}

fn builtin_null(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Null)
}

fn builtin_isnull(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(matches!(args[0], Value::Null)))
}

fn builtin_typeof(args: &[Value]) -> Result<Value, RuntimeError> {
    let name = match &args[0] {
        Value::Null => "NULL",
        Value::Numeric(_) => "NUMERIC",
        Value::Integer(_) => "INTEGER",
        Value::Long(_) => "LONG",
        Value::Single(_) => "SINGLE",
        Value::Int64(_) => "INT64",
        Value::Bool(_) => "BOOL",
        Value::Byte(_) => "BYTE",
        Value::Bytes(_) => "BYTES",
        Value::Str(_) => "STRING",
        Value::Record { type_name, .. } => type_name,
    };
    Ok(Value::Str(name.into()))
}

fn byte_buffer(value: &Value) -> Result<&[u8], RuntimeError> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(RuntimeError::TypeMismatch {
            msg: "expected a byte buffer".into(),
        }),
    }
}

fn buffer_allocation_error() -> RuntimeError {
    RuntimeError::Overflow {
        msg: "byte buffer exceeds available memory".into(),
    }
}

fn allocate_buffer(length: usize) -> Result<Vec<u8>, RuntimeError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| buffer_allocation_error())?;
    Ok(bytes)
}

fn copy_buffer(source: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let mut bytes = allocate_buffer(source.len())?;
    bytes.extend_from_slice(source);
    Ok(bytes)
}

fn builtin_bytes(args: &[Value]) -> Result<Value, RuntimeError> {
    let mut bytes = allocate_buffer(args.len())?;
    for value in args {
        bytes.push(exact_byte(value)?);
    }
    Ok(Value::Bytes(bytes))
}

fn builtin_bytebuffer(args: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&args.len()) {
        return Err(RuntimeError::ArityMismatch {
            expected: 1,
            got: args.len(),
        });
    }
    let number = args[0].to_exact_i64()?;
    let length = usize::try_from(number).map_err(|_| RuntimeError::IllegalFunctionCall {
        msg: "BYTEBUFFER length must be non-negative and fit in memory".into(),
    })?;
    let fill = if args.len() == 2 {
        exact_byte(&args[1])?
    } else {
        0
    };
    let mut bytes = allocate_buffer(length)?;
    bytes.resize(length, fill);
    Ok(Value::Bytes(bytes))
}

fn builtin_bytelen(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Numeric(byte_buffer(&args[0])?.len() as f64))
}

fn buffer_index(value: &Value, length: usize, allow_end: bool) -> Result<usize, RuntimeError> {
    let number = value.to_exact_i64()?;
    let index = number.checked_sub(1).and_then(|n| usize::try_from(n).ok());
    match index {
        Some(index) if index < length || (allow_end && index == length) => Ok(index),
        _ => Err(RuntimeError::IllegalFunctionCall {
            msg: "byte buffer index is outside its 1-based bounds".into(),
        }),
    }
}

fn builtin_byteat(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = byte_buffer(&args[0])?;
    let index = buffer_index(&args[1], bytes.len(), false)?;
    Ok(Value::Byte(bytes[index]))
}

fn builtin_byteset(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = byte_buffer(&args[0])?;
    let index = buffer_index(&args[1], bytes.len(), false)?;
    let value = exact_byte(&args[2])?;
    let mut result = copy_buffer(bytes)?;
    result[index] = value;
    Ok(Value::Bytes(result))
}

fn builtin_byteslice(args: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&args.len()) {
        return Err(RuntimeError::ArityMismatch {
            expected: 2,
            got: args.len(),
        });
    }
    let bytes = byte_buffer(&args[0])?;
    let start = buffer_index(&args[1], bytes.len(), true)?;
    let remaining = bytes.len() - start;
    let count = if args.len() == 3 {
        let number = args[2].to_exact_i64()?;
        if number < 0 {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: "BYTESLICE count must be non-negative".into(),
            });
        }
        usize::try_from(number).unwrap_or(usize::MAX).min(remaining)
    } else {
        remaining
    };
    Ok(Value::Bytes(copy_buffer(&bytes[start..start + count])?))
}

fn builtin_utf8encode(args: &[Value]) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Str(text) => Ok(Value::Bytes(copy_buffer(text.as_bytes())?)),
        _ => Err(RuntimeError::TypeMismatch {
            msg: "UTF8ENCODE expects a string".into(),
        }),
    }
}

fn builtin_utf8decode(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = byte_buffer(&args[0])?;
    // Validate before allocating; malformed text must never silently replace bytes.
    std::str::from_utf8(bytes).map_err(|_| RuntimeError::IllegalFunctionCall {
        msg: "UTF8DECODE$: byte buffer is not valid UTF-8".into(),
    })?;
    Ok(Value::Str(
        String::from_utf8(copy_buffer(bytes)?).expect("validated UTF-8"),
    ))
}

fn builtin_hexencode(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = byte_buffer(&args[0])?;
    let length = bytes
        .len()
        .checked_mul(2)
        .ok_or_else(buffer_allocation_error)?;
    let mut output = String::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| buffer_allocation_error())?;
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 15) as usize] as char);
    }
    Ok(Value::Str(output))
}

fn builtin_hexdecode(args: &[Value]) -> Result<Value, RuntimeError> {
    let text = match &args[0] {
        Value::Str(text) => text,
        _ => {
            return Err(RuntimeError::TypeMismatch {
                msg: "HEXDECODE expects a string".into(),
            });
        }
    };
    if text.len() % 2 != 0 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "HEXDECODE expects an even number of hexadecimal digits".into(),
        });
    }
    let mut output = allocate_buffer(text.len() / 2)?;
    let digit = |byte: u8| {
        if byte <= b'9' {
            byte - b'0'
        } else {
            byte.to_ascii_uppercase() - b'A' + 10
        }
    };
    for pair in text.as_bytes().chunks_exact(2) {
        output.push((digit(pair[0]) << 4) | digit(pair[1]));
    }
    Ok(Value::Bytes(output))
}

// Math builtins

fn builtin_abs(args: &[Value]) -> Result<Value, RuntimeError> {
    if let Value::Integer(n) = args[0] {
        return n
            .checked_abs()
            .map(Value::Integer)
            .ok_or_else(|| RuntimeError::Overflow {
                msg: "ABS: result exceeds the INTEGER range".into(),
            });
    }
    if let Value::Long(n) = args[0] {
        return n
            .checked_abs()
            .map(Value::Long)
            .ok_or_else(|| RuntimeError::Overflow {
                msg: "ABS: result exceeds the LONG range".into(),
            });
    }
    if let Value::Single(n) = args[0] {
        return Ok(Value::Single(n.abs()));
    }
    if let Value::Int64(n) = args[0] {
        return n
            .checked_abs()
            .map(Value::Int64)
            .ok_or_else(|| RuntimeError::Overflow {
                msg: "ABS: result exceeds the INT64 range".into(),
            });
    }
    if let Value::Byte(n) = args[0] {
        return Ok(Value::Byte(n));
    }
    let n = args[0].to_f64()?;
    Ok(Value::Numeric(n.abs()))
}

fn builtin_int(args: &[Value]) -> Result<Value, RuntimeError> {
    if args[0].integer_value().is_some() {
        return Ok(args[0].clone());
    }
    if let Value::Single(n) = args[0] {
        return Ok(Value::Single(n.floor()));
    }
    let n = args[0].to_f64()?;
    Ok(Value::Numeric(n.floor()))
}

fn builtin_fix(args: &[Value]) -> Result<Value, RuntimeError> {
    if args[0].integer_value().is_some() {
        return Ok(args[0].clone());
    }
    if let Value::Single(n) = args[0] {
        return Ok(Value::Single(n.trunc()));
    }
    let n = args[0].to_f64()?;
    Ok(Value::Numeric(n.trunc()))
}

/// Round half-to-even (banker's rounding) for CINT and CLNG.
fn round_half_even(n: f64) -> f64 {
    let rounded = n.round();
    if (n - n.trunc()).abs() == 0.5 {
        let floor = n.floor();
        if (floor as i64) % 2 == 0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        rounded
    }
}

fn builtin_cint(args: &[Value]) -> Result<Value, RuntimeError> {
    rounded_integer(&args[0])?.coerce_to(BasicType::Integer)
}

fn builtin_clng(args: &[Value]) -> Result<Value, RuntimeError> {
    rounded_integer(&args[0])?.coerce_to(BasicType::Long)
}

fn rounded_integer(value: &Value) -> Result<Value, RuntimeError> {
    if let Value::Bool(boolean) = value {
        return Ok(Value::Int64(i64::from(*boolean)));
    }
    if value.integer_value().is_some() {
        return Ok(value.clone());
    }
    Ok(Value::Numeric(round_half_even(value.to_f64()?)))
}

fn builtin_csng(args: &[Value]) -> Result<Value, RuntimeError> {
    if let Value::Bool(boolean) = args[0] {
        return Ok(Value::Single(u8::from(boolean) as f32));
    }
    Ok(Value::Single(args[0].to_f32()?))
}

fn builtin_cdbl(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = match args[0] {
        Value::Bool(value) => {
            if value {
                1.0
            } else {
                0.0
            }
        }
        _ => args[0].to_f64_lossy()?,
    };
    Ok(Value::Numeric(n))
}

fn builtin_sgn(args: &[Value]) -> Result<Value, RuntimeError> {
    if let Some(n) = args[0].integer_value() {
        return Ok(Value::Int64(n.signum()));
    }
    let n = args[0].to_f64()?;
    let s = if n > 0.0 {
        1
    } else if n < 0.0 {
        -1
    } else {
        0
    };
    Ok(Value::Numeric(s as f64))
}

fn builtin_sqr(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    if n < 0.0 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "SQR of negative number".into(),
        });
    }
    Ok(Value::Numeric(n.sqrt()))
}

fn builtin_sin(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Numeric(args[0].to_f64()?.sin()))
}

fn builtin_cos(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Numeric(args[0].to_f64()?.cos()))
}

fn builtin_tan(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Numeric(args[0].to_f64()?.tan()))
}

fn builtin_atn(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Numeric(args[0].to_f64()?.atan()))
}

fn builtin_exp(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Numeric(args[0].to_f64()?.exp()))
}

fn builtin_log(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    if n <= 0.0 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "LOG of non-positive number".into(),
        });
    }
    Ok(Value::Numeric(n.ln()))
}

fn builtin_round(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args
        .first()
        .ok_or(RuntimeError::ArityMismatch {
            expected: 1,
            got: 0,
        })?
        .to_f64()?;
    match args.len() {
        1 => Ok(Value::Numeric(n.round())),
        2 => {
            let places = args[1].to_i64()?;
            if places < 0 {
                return round_negative_places(n, places, f64::round);
            }
            let factor = decimal_factor(places)?;
            let scaled = n * factor;
            // Scaling overflow means this precision cannot change the stored value.
            Ok(Value::Numeric(if scaled.is_finite() {
                scaled.round() / factor
            } else {
                n
            }))
        }
        _ => Err(RuntimeError::ArityMismatch {
            expected: 2,
            got: args.len(),
        }),
    }
}

fn builtin_asin(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    if !(-1.0..=1.0).contains(&n) {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "ASIN argument out of range".into(),
        });
    }
    Ok(Value::Numeric(n.asin()))
}

fn builtin_acos(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    if !(-1.0..=1.0).contains(&n) {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "ACOS argument out of range".into(),
        });
    }
    Ok(Value::Numeric(n.acos()))
}

fn builtin_cot(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    let t = n.tan();
    if t == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    Ok(Value::Numeric(1.0 / t))
}

fn builtin_csc(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    let s = n.sin();
    if s == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    Ok(Value::Numeric(1.0 / s))
}

fn builtin_sec(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    let c = n.cos();
    if c == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    Ok(Value::Numeric(1.0 / c))
}

fn builtin_angle(args: &[Value]) -> Result<Value, RuntimeError> {
    let x = args[0].to_f64()?;
    let y = args[1].to_f64()?;
    Ok(Value::Numeric(y.atan2(x)))
}

fn builtin_ceil(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Numeric(args[0].to_f64()?.ceil()))
}

fn builtin_truncate(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args
        .first()
        .ok_or(RuntimeError::ArityMismatch {
            expected: 1,
            got: 0,
        })?
        .to_f64()?;
    match args.len() {
        1 => Ok(Value::Numeric(n.trunc())),
        2 => {
            let places = args[1].to_i64()?;
            if places < 0 {
                return round_negative_places(n, places, f64::trunc);
            }
            let factor = decimal_factor(places)?;
            let scaled = n * factor;
            Ok(Value::Numeric(if scaled.is_finite() {
                scaled.trunc() / factor
            } else {
                n
            }))
        }
        _ => Err(RuntimeError::ArityMismatch {
            expected: 2,
            got: args.len(),
        }),
    }
}

fn builtin_remainder(args: &[Value]) -> Result<Value, RuntimeError> {
    let a = args[0].to_f64()?;
    let b = args[1].to_f64()?;
    if b == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    Ok(Value::Numeric(a % b))
}

fn decimal_factor(places: i64) -> Result<f64, RuntimeError> {
    if !(0..=308).contains(&places) {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "decimal places out of range".into(),
        });
    }
    Ok(10_f64.powi(places as i32))
}

fn round_negative_places(
    number: f64,
    places: i64,
    operation: fn(f64) -> f64,
) -> Result<Value, RuntimeError> {
    if !(-308..=-1).contains(&places) {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "decimal places out of range".into(),
        });
    }
    if !number.is_finite() {
        return Ok(Value::Numeric(number));
    }
    // Parsing a decimal power avoids accumulated powi error: truncating exactly
    // 1e308 to 10^308 must not see a quotient slightly less than one.
    let factor = format!("1e{}", -places)
        .parse::<f64>()
        .expect("a decimal exponent in 1..308 is finite and valid");
    let result = operation(number / factor) * factor;
    if !result.is_finite() {
        return Err(RuntimeError::Overflow {
            msg: "rounded value exceeds the numeric range".into(),
        });
    }
    Ok(Value::Numeric(result))
}

fn builtin_min(args: &[Value]) -> Result<Value, RuntimeError> {
    numeric_extremum(args, false)
}

fn builtin_max(args: &[Value]) -> Result<Value, RuntimeError> {
    numeric_extremum(args, true)
}

fn numeric_extremum(args: &[Value], maximum: bool) -> Result<Value, RuntimeError> {
    let mut result = args
        .first()
        .ok_or(RuntimeError::ArityMismatch {
            expected: 1,
            got: 0,
        })?
        .clone();
    if !result.is_numeric() {
        return Err(RuntimeError::TypeMismatch {
            msg: "MIN/MAX expects numeric values".into(),
        });
    }
    for arg in &args[1..] {
        if !arg.is_numeric() {
            return Err(RuntimeError::TypeMismatch {
                msg: "MIN/MAX expects numeric values".into(),
            });
        }
        // Propagate NaN, but continue validating the remaining arguments.
        if result.floating_value().is_some_and(f64::is_nan)
            || arg.floating_value().is_some_and(f64::is_nan)
        {
            result = Value::Numeric(f64::NAN);
        } else if let (Value::Numeric(a), Value::Numeric(b)) = (&result, arg) {
            result = Value::Numeric(if maximum { a.max(*b) } else { a.min(*b) });
        } else if matches!(arg.partial_cmp(&result), Some(order) if
            (maximum && order.is_gt()) || (!maximum && order.is_lt()))
        {
            result = arg.clone();
        }
    }
    Ok(result)
}

fn builtin_maxnum(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Numeric(f64::MAX))
}

fn builtin_pi(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Numeric(std::f64::consts::PI))
}

// String builtins

fn builtin_len(args: &[Value]) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Str(s) => Ok(Value::Numeric(s.chars().count() as f64)),
        Value::Bytes(bytes) => Ok(Value::Numeric(bytes.len() as f64)),
        _ => Err(RuntimeError::TypeMismatch {
            msg: "LEN expects a string or byte buffer".into(),
        }),
    }
}

fn builtin_instr(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.len() {
        2 => {
            let haystack = args[0].to_string_val()?;
            let needle = args[1].to_string_val()?;
            let pos = haystack
                .find(&needle)
                .map(|p| haystack[..p].chars().count() + 1)
                .unwrap_or(0);
            Ok(Value::Numeric(pos as f64))
        }
        3 => {
            let start = args[0].to_i64()?;
            if start < 1 {
                return Err(RuntimeError::IllegalFunctionCall {
                    msg: "INSTR start must be >= 1".into(),
                });
            }
            let start = (start - 1) as usize;
            let haystack = args[1].to_string_val()?;
            let needle = args[2].to_string_val()?;
            let Some((byte_start, _)) = haystack.char_indices().nth(start) else {
                return Ok(Value::Numeric(0.0));
            };
            let suffix = &haystack[byte_start..];
            let pos = suffix
                .find(&needle)
                .map(|byte_pos| start + suffix[..byte_pos].chars().count() + 1)
                .unwrap_or(0);
            Ok(Value::Numeric(pos as f64))
        }
        _ => Err(RuntimeError::ArityMismatch {
            expected: 2,
            got: args.len(),
        }),
    }
}

fn builtin_strcomp(args: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&args.len()) {
        return Err(RuntimeError::ArityMismatch {
            expected: 2,
            got: args.len(),
        });
    }
    let mut left = args[0].to_string_val()?;
    let mut right = args[1].to_string_val()?;
    let compare = args.get(2).map(Value::to_f64).transpose()?.unwrap_or(0.0);
    match compare {
        0.0 => {}
        1.0 => {
            left = left.to_lowercase();
            right = right.to_lowercase();
        }
        _ => {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: "STRCOMP comparison mode must be 0 (binary) or 1 (Unicode lowercase)".into(),
            });
        }
    }
    let result = match left.cmp(&right) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    };
    Ok(Value::Numeric(result))
}

fn builtin_instrrev(args: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&args.len()) {
        return Err(RuntimeError::ArityMismatch {
            expected: 2,
            got: args.len(),
        });
    }
    let haystack = args[0].to_string_val()?;
    let needle = args[1].to_string_val()?;
    let start = args.get(2).map(Value::to_i64).transpose()?.unwrap_or(-1);
    if start == 0 || start < -1 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "INSTRREV start must be -1 or at least 1".into(),
        });
    }
    let length = haystack.chars().count();
    let end = if start == -1 {
        length
    } else if let Ok(end) = usize::try_from(start) {
        end
    } else {
        return Ok(Value::Numeric(0.0));
    };
    if length == 0 || end > length {
        return Ok(Value::Numeric(0.0));
    }
    if needle.is_empty() {
        return Ok(Value::Numeric(end as f64));
    }
    let byte_end = haystack
        .char_indices()
        .nth(end)
        .map_or(haystack.len(), |(index, _)| index);
    let prefix = &haystack[..byte_end];
    let position = prefix
        .rfind(&needle)
        .map_or(0, |index| prefix[..index].chars().count() + 1);
    Ok(Value::Numeric(position as f64))
}

fn builtin_replace(args: &[Value]) -> Result<Value, RuntimeError> {
    if !(3..=5).contains(&args.len()) {
        return Err(RuntimeError::ArityMismatch {
            expected: 3,
            got: args.len(),
        });
    }
    let text = args[0].to_string_val()?;
    let find = args[1].to_string_val()?;
    let replacement = args[2].to_string_val()?;
    let start = args.get(3).map(Value::to_i64).transpose()?.unwrap_or(1);
    let count = args.get(4).map(Value::to_i64).transpose()?.unwrap_or(-1);
    if start < 1 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "REPLACE$ start must be at least 1".into(),
        });
    }
    if count < -1 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "REPLACE$ count must be -1 or nonnegative".into(),
        });
    }
    let byte_start = usize::try_from(start - 1)
        .ok()
        .and_then(|index| text.char_indices().nth(index))
        .map_or(text.len(), |(index, _)| index);
    let suffix = &text[byte_start..];
    if find.is_empty() || count == 0 {
        return Ok(Value::Str(suffix.to_string()));
    }
    let result = if count == -1 {
        suffix.replace(&find, &replacement)
    } else {
        suffix.replacen(
            &find,
            &replacement,
            usize::try_from(count).unwrap_or(usize::MAX),
        )
    };
    Ok(Value::Str(result))
}

fn builtin_ltrim(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Str(
        args[0].to_string_val()?.trim_start().to_string(),
    ))
}

fn builtin_rtrim(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Str(args[0].to_string_val()?.trim_end().to_string()))
}

fn builtin_space(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()?;
    if n < 0 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "SPACE$ argument must be non-negative".into(),
        });
    }
    Ok(Value::Str(" ".repeat(n as usize)))
}

fn builtin_string_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()?;
    if n < 0 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "STRING$ count must be non-negative".into(),
        });
    }
    let n = n as usize;
    let ch = match &args[1] {
        Value::Str(s) => s.chars().next().unwrap_or(' '),
        v => char::from(v.to_i64()? as u8),
    };
    Ok(Value::Str(ch.to_string().repeat(n)))
}

fn builtin_chr(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()?;
    if !(0..=255).contains(&n) {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "CHR$ argument out of range".into(),
        });
    }
    Ok(Value::Str(String::from(n as u8 as char)))
}

fn builtin_asc(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let first = s
        .chars()
        .next()
        .ok_or_else(|| RuntimeError::IllegalFunctionCall {
            msg: "ASC of empty string".into(),
        })?;
    Ok(Value::Numeric(first as u32 as f64))
}

fn builtin_str(args: &[Value]) -> Result<Value, RuntimeError> {
    if let Value::Single(number) = args[0] {
        return Ok(Value::Str(number.to_string()));
    }
    if let Some(n) = args[0].integer_value() {
        return Ok(Value::Str(n.to_string()));
    }
    if let Value::Bool(value) = args[0] {
        return Ok(Value::Str(if value { "TRUE" } else { "FALSE" }.into()));
    }
    let n = args[0].to_f64()?;
    let formatted = if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    };
    Ok(Value::Str(formatted))
}

fn builtin_val(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let s = s.trim();
    if s.is_empty() {
        return Ok(Value::Numeric(0.0));
    }
    // Hexadecimal (&H) and octal (&O) literals
    if let Some(rest) = s.strip_prefix("&H").or_else(|| s.strip_prefix("&h")) {
        let hex: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        let n = i64::from_str_radix(&hex, 16).unwrap_or(0);
        return Ok(Value::Numeric(n as f64));
    }
    if let Some(rest) = s.strip_prefix("&O").or_else(|| s.strip_prefix("&o")) {
        let oct: String = rest
            .chars()
            .take_while(|c| ('0'..='7').contains(c))
            .collect();
        let n = i64::from_str_radix(&oct, 8).unwrap_or(0);
        return Ok(Value::Numeric(n as f64));
    }
    // Try integer first, then float
    if let Ok(n) = s.parse::<i64>() {
        return Ok(Value::Numeric(n as f64));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(Value::Numeric(n));
    }
    // Scan the longest valid decimal prefix, including BASIC's D exponent.
    // All scanned bytes are ASCII, so stopping before Unicode text is safe.
    let bytes = s.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let integer_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let mut digits = end - integer_start;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let fractional_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        digits += end - fractional_start;
    }
    if digits == 0 {
        return Ok(Value::Numeric(0.0));
    }
    let mantissa_end = end;
    if matches!(bytes.get(end), Some(b'E' | b'e' | b'D' | b'd')) {
        end += 1;
        if matches!(bytes.get(end), Some(b'+' | b'-')) {
            end += 1;
        }
        let exponent_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end == exponent_start {
            end = mantissa_end;
        }
    }
    let prefix = s[..end].replace(['D', 'd'], "E");
    Ok(Value::Numeric(prefix.parse::<f64>().unwrap_or(0.0)))
}

fn builtin_left(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let n = args[1].to_i64()?;
    if n < 0 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "LEFT$ argument must be non-negative".into(),
        });
    }
    let result: String = s.chars().take(n as usize).collect();
    Ok(Value::Str(result))
}

fn builtin_right(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let n = args[1].to_i64()?;
    if n < 0 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "RIGHT$ argument must be non-negative".into(),
        });
    }
    let char_count = s.chars().count();
    let skip = char_count.saturating_sub(n as usize);
    let result: String = s.chars().skip(skip).collect();
    Ok(Value::Str(result))
}

fn builtin_mid(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.len() {
        2 => {
            let s = args[0].to_string_val()?;
            let start = args[1].to_i64()?;
            if start < 1 {
                return Err(RuntimeError::IllegalFunctionCall {
                    msg: "MID$ start must be >= 1".into(),
                });
            }
            let result: String = s.chars().skip((start - 1) as usize).collect();
            Ok(Value::Str(result))
        }
        3 => {
            let s = args[0].to_string_val()?;
            let start = args[1].to_i64()?;
            let length = args[2].to_i64()?;
            if start < 1 {
                return Err(RuntimeError::IllegalFunctionCall {
                    msg: "MID$ start must be >= 1".into(),
                });
            }
            if length < 0 {
                return Err(RuntimeError::IllegalFunctionCall {
                    msg: "MID$ length must be non-negative".into(),
                });
            }
            let result: String = s
                .chars()
                .skip((start - 1) as usize)
                .take(length as usize)
                .collect();
            Ok(Value::Str(result))
        }
        _ => Err(RuntimeError::IllegalFunctionCall {
            msg: format!("MID$ requires 2 or 3 arguments, got {}", args.len()),
        }),
    }
}

fn builtin_ucase(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    Ok(Value::Str(s.to_uppercase()))
}

fn builtin_lcase(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    Ok(Value::Str(s.to_lowercase()))
}

fn builtin_hex(args: &[Value]) -> Result<Value, RuntimeError> {
    let i = match args[0].integer_value() {
        Some(n) => n,
        None => args[0].to_f64()? as i64,
    };
    if i < 0 {
        Ok(Value::Str(format!("{:X}", i as u64)))
    } else {
        Ok(Value::Str(format!("{:X}", i)))
    }
}

fn builtin_oct(args: &[Value]) -> Result<Value, RuntimeError> {
    let i = match args[0].integer_value() {
        Some(n) => n,
        None => args[0].to_f64()? as i64,
    };
    if i < 0 {
        Ok(Value::Str(format!("{:o}", i as u64)))
    } else {
        Ok(Value::Str(format!("{:o}", i)))
    }
}

fn builtin_mki(args: &[Value]) -> Result<Value, RuntimeError> {
    let Value::Integer(n) = args[0].coerce_to(BasicType::Integer)? else {
        unreachable!()
    };
    Ok(Value::Str(bytes_to_basic_string(&n.to_le_bytes())))
}

fn builtin_mkl(args: &[Value]) -> Result<Value, RuntimeError> {
    let Value::Long(n) = args[0].coerce_to(BasicType::Long)? else {
        unreachable!()
    };
    Ok(Value::Str(bytes_to_basic_string(&n.to_le_bytes())))
}

fn builtin_mks(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f32()?;
    Ok(Value::Str(bytes_to_basic_string(&n.to_le_bytes())))
}

fn builtin_mkd(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(Value::Str(bytes_to_basic_string(&n.to_le_bytes())))
}

fn require_packed_bytes(args: &[Value], name: &str, len: usize) -> Result<Vec<u8>, RuntimeError> {
    let s = args[0].to_string_val()?;
    let bytes = basic_string_to_bytes(&s);
    if bytes.len() < len {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: format!("{name} requires at least {len} bytes"),
        });
    }
    Ok(bytes)
}

fn builtin_cvi(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = require_packed_bytes(args, "CVI", 2)?;
    Ok(Value::Integer(i16::from_le_bytes([bytes[0], bytes[1]])))
}

fn builtin_cvl(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = require_packed_bytes(args, "CVL", 4)?;
    Ok(Value::Long(i32::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3],
    ])))
}

fn builtin_cvs(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = require_packed_bytes(args, "CVS", 4)?;
    Ok(Value::Single(f32::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3],
    ])))
}

fn builtin_cvd(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = require_packed_bytes(args, "CVD", 8)?;
    Ok(Value::Numeric(f64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])))
}

/// Get local time components (hours, minutes, seconds, millis) from system time.
/// Uses platform-specific APIs to get local time without external dependencies.
fn local_time_parts() -> (u64, u64, u64, u32) {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct SystemTime {
            year: u16,
            month: u16,
            _dow: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            millis: u16,
        }
        unsafe extern "system" {
            fn GetLocalTime(st: *mut SystemTime);
        }
        let mut st = SystemTime {
            year: 0,
            month: 0,
            _dow: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
            millis: 0,
        };
        unsafe {
            GetLocalTime(&mut st);
        }
        (
            st.hour as u64,
            st.minute as u64,
            st.second as u64,
            st.millis as u32,
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        #[repr(C)]
        struct Tm {
            tm_sec: i32,
            tm_min: i32,
            tm_hour: i32,
            tm_mday: i32,
            tm_mon: i32,
            tm_year: i32,
            tm_wday: i32,
            tm_yday: i32,
            tm_isdst: i32,
            tm_gmtoff: i64,
            tm_zone: *const i8,
        }
        unsafe extern "C" {
            fn time(t: *mut i64) -> i64;
            fn localtime_r(t: *const i64, result: *mut Tm) -> *mut Tm;
        }
        let mut t: i64 = 0;
        let mut tm = Tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 0,
            tm_mon: 0,
            tm_year: 0,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: std::ptr::null(),
        };
        unsafe {
            time(&mut t);
            localtime_r(&t, &mut tm);
        }
        (tm.tm_hour as u64, tm.tm_min as u64, tm.tm_sec as u64, 0)
    }
}

/// Get local date components (year, month, day).
fn local_date_parts() -> (u16, u16, u16) {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct SystemTime {
            year: u16,
            month: u16,
            _dow: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            millis: u16,
        }
        unsafe extern "system" {
            fn GetLocalTime(st: *mut SystemTime);
        }
        let mut st = SystemTime {
            year: 0,
            month: 0,
            _dow: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
            millis: 0,
        };
        unsafe {
            GetLocalTime(&mut st);
        }
        (st.year, st.month, st.day)
    }
    #[cfg(not(target_os = "windows"))]
    {
        #[repr(C)]
        struct Tm {
            tm_sec: i32,
            tm_min: i32,
            tm_hour: i32,
            tm_mday: i32,
            tm_mon: i32,
            tm_year: i32,
            tm_wday: i32,
            tm_yday: i32,
            tm_isdst: i32,
            tm_gmtoff: i64,
            tm_zone: *const i8,
        }
        unsafe extern "C" {
            fn time(t: *mut i64) -> i64;
            fn localtime_r(t: *const i64, result: *mut Tm) -> *mut Tm;
        }
        let mut t: i64 = 0;
        let mut tm = Tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 0,
            tm_mon: 0,
            tm_year: 0,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: std::ptr::null(),
        };
        unsafe {
            time(&mut t);
            localtime_r(&t, &mut tm);
        }
        (
            (tm.tm_year + 1900) as u16,
            (tm.tm_mon + 1) as u16,
            tm.tm_mday as u16,
        )
    }
}

fn builtin_timer(_args: &[Value]) -> Result<Value, RuntimeError> {
    let (h, m, s, ms) = local_time_parts();
    let secs_today = h * 3600 + m * 60 + s;
    Ok(Value::Numeric(secs_today as f64 + ms as f64 / 1000.0))
}

pub(crate) fn builtin_date(_args: &[Value]) -> Result<Value, RuntimeError> {
    let (year, month, day) = local_date_parts();
    Ok(Value::Str(format!("{:02}-{:02}-{:04}", month, day, year)))
}

pub(crate) fn builtin_time(_args: &[Value]) -> Result<Value, RuntimeError> {
    let (hours, mins, secs, _) = local_time_parts();
    Ok(Value::Str(format!("{:02}:{:02}:{:02}", hours, mins, secs)))
}

fn builtin_environ(args: &[Value]) -> Result<Value, RuntimeError> {
    if args[0].is_numeric() {
        return indexed_environment_value(&args[0], &HashMap::new());
    }
    let name = args[0].to_string_val()?;
    let val = std::env::var(&name).unwrap_or_default();
    Ok(Value::Str(val))
}

fn ordered_environment_entries(
    host: impl IntoIterator<Item = (OsString, OsString)>,
    overrides: &HashMap<String, String>,
) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    for (name, value) in host {
        if let (Ok(name), Ok(value)) = (name.into_string(), value.into_string()) {
            let name = if cfg!(windows) {
                name.to_uppercase()
            } else {
                name
            };
            entries.insert(name, value);
        }
    }
    for (name, value) in overrides {
        let name = if cfg!(windows) {
            name.to_uppercase()
        } else {
            name.clone()
        };
        entries.insert(name, value.clone());
    }
    entries
}

pub(crate) fn indexed_environment_value(
    index: &Value,
    overrides: &HashMap<String, String>,
) -> Result<Value, RuntimeError> {
    let index = index.to_i64()?;
    if index < 1 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "ENVIRON$ index must be at least 1".into(),
        });
    }
    let Ok(index) = usize::try_from(index - 1) else {
        return Ok(Value::Str(String::new()));
    };
    let entries = ordered_environment_entries(std::env::vars_os(), overrides);
    let entry = entries
        .into_iter()
        .nth(index)
        .map(|(name, value)| format!("{name}={value}"));
    Ok(Value::Str(entry.unwrap_or_default()))
}

fn builtin_curdir(_args: &[Value]) -> Result<Value, RuntimeError> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(Value::Str(cwd))
}

fn builtin_command(_args: &[Value]) -> Result<Value, RuntimeError> {
    // Interpreter calls use explicitly configured program arguments. A standalone
    // registry has no program context and must not expose the host's process args.
    Ok(Value::Str(String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call_native(name: &str, args: &[Value]) -> Value {
        BuiltinRegistry::new().call(name, args).unwrap().unwrap()
    }

    #[test]
    fn null_builtins_preserve_missing_values_and_check_arity() {
        let registry = BuiltinRegistry::new();
        assert!(registry.exists("null"));
        assert!(registry.exists("isnull"));
        assert_eq!(call_native("null", &[]), Value::Null);
        assert_eq!(call_native("isnull", &[Value::Null]), Value::Bool(true));
        for value in [
            Value::Numeric(0.0),
            Value::Bool(false),
            Value::Str(String::new()),
            Value::Bytes(Vec::new()),
        ] {
            assert_eq!(call_native("ISNULL", &[value]), Value::Bool(false));
        }
        for (name, args) in [
            ("NULL", vec![Value::Numeric(0.0)]),
            ("ISNULL", vec![]),
            ("ISNULL", vec![Value::Null, Value::Null]),
        ] {
            assert!(matches!(
                registry.call(name, &args),
                Err(RuntimeError::ArityMismatch { .. })
            ));
        }
        for name in [
            "CBOOL", "CBYTE", "CINT", "CLNG", "CINT64", "CSNG", "CDBL", "STR$",
        ] {
            assert!(matches!(
                registry.call(name, &[Value::Null]),
                Err(RuntimeError::TypeMismatch { .. })
            ));
        }
    }

    #[test]
    fn exact_integer_conversions_preserve_boundaries() {
        for number in [i64::MIN, i64::MAX, 9_007_199_254_740_993, 0, -1] {
            let exact = Value::Int64(number);
            assert_eq!(
                call_native("CINT64", &[Value::Str(number.to_string())]),
                exact
            );
            assert_eq!(call_native("CINT64", &[exact.clone()]), exact);
            assert_eq!(
                call_native("STR$", &[exact.clone()]),
                Value::Str(number.to_string())
            );
            assert_eq!(call_native("INT", &[exact.clone()]), exact);
            assert_eq!(call_native("FIX", &[exact.clone()]), exact);
            assert_eq!(call_native("CDBL", &[exact]), Value::Numeric(number as f64));
        }
        assert_eq!(
            call_native("CINT64", &[Value::Str(" +42 ".into())]),
            Value::Int64(42)
        );
        assert_eq!(
            call_native("CINT64", &[Value::Numeric(42.0)]),
            Value::Int64(42)
        );
        let registry = BuiltinRegistry::new();
        for invalid in [
            Value::Str("9223372036854775808".into()),
            Value::Str("-9223372036854775809".into()),
            Value::Str("3.0".into()),
            Value::Str("1e2".into()),
            Value::Str("".into()),
            Value::Numeric(1.5),
            Value::Numeric(f64::NAN),
            Value::Numeric(f64::INFINITY),
            Value::Numeric(9_223_372_036_854_775_808.0),
        ] {
            assert!(registry.call("CINT64", &[invalid]).is_err());
        }
    }

    #[test]
    fn boolean_and_byte_conversions_are_explicit_and_checked() {
        assert_eq!(call_native("TRUE", &[]), Value::Bool(true));
        assert_eq!(call_native("FALSE", &[]), Value::Bool(false));
        for (value, expected) in [
            (Value::Numeric(-0.0), false),
            (Value::Numeric(-0.25), true),
            (Value::Int64(i64::MAX), true),
            (Value::Byte(0), false),
            (Value::Bool(true), true),
        ] {
            assert_eq!(call_native("CBOOL", &[value]), Value::Bool(expected));
        }
        for (flag, number) in [(false, 0), (true, 1)] {
            assert_eq!(
                call_native("CINT64", &[Value::Bool(flag)]),
                Value::Int64(number)
            );
            assert_eq!(
                call_native("CBYTE", &[Value::Bool(flag)]),
                Value::Byte(number as u8)
            );
            assert_eq!(
                call_native("CDBL", &[Value::Bool(flag)]),
                Value::Numeric(number as f64)
            );
        }
        for number in 0..=255 {
            assert_eq!(
                call_native("CBYTE", &[Value::Numeric(number as f64)]),
                Value::Byte(number)
            );
        }
        let registry = BuiltinRegistry::new();
        for invalid in [
            Value::Numeric(-1.0),
            Value::Numeric(256.0),
            Value::Numeric(1.5),
            Value::Int64(i64::MAX),
            Value::Str("1".into()),
        ] {
            assert!(registry.call("CBYTE", &[invalid]).is_err());
        }
        for invalid in [
            Value::Numeric(f64::NAN),
            Value::Numeric(f64::INFINITY),
            Value::Str("TRUE".into()),
            Value::Bytes(vec![]),
        ] {
            assert!(registry.call("CBOOL", &[invalid]).is_err());
        }
    }

    #[test]
    fn type_names_reflect_native_runtime_values() {
        for (value, name) in [
            (Value::Null, "NULL"),
            (Value::Numeric(1.0), "NUMERIC"),
            (Value::Integer(1), "INTEGER"),
            (Value::Long(1), "LONG"),
            (Value::Single(1.0), "SINGLE"),
            (Value::Int64(1), "INT64"),
            (Value::Bool(true), "BOOL"),
            (Value::Byte(1), "BYTE"),
            (Value::Bytes(vec![]), "BYTES"),
            (Value::Str(String::new()), "STRING"),
            (
                Value::Record {
                    type_name: "POINT".into(),
                    fields: HashMap::new(),
                },
                "POINT",
            ),
        ] {
            assert_eq!(call_native("TYPEOF$", &[value]), Value::Str(name.into()));
        }
    }

    #[test]
    fn fixed_width_converters_return_typed_values_and_check_rounded_ranges() {
        for (number, expected) in [
            (2.5, 2),
            (3.5, 4),
            (-2.5, -2),
            (-3.5, -4),
            (32767.49, 32767),
            (-32768.5, -32768),
        ] {
            assert!(
                matches!(call_native("CINT", &[Value::Numeric(number)]), Value::Integer(n) if n == expected)
            );
        }
        for (number, expected) in [
            (2.5, 2),
            (3.5, 4),
            (-2.5, -2),
            (2_147_483_647.49, i32::MAX),
            (-2_147_483_648.5, i32::MIN),
        ] {
            assert!(
                matches!(call_native("CLNG", &[Value::Numeric(number)]), Value::Long(n) if n == expected)
            );
        }
        let registry = BuiltinRegistry::new();
        for (name, number) in [
            ("CINT", 32767.5),
            ("CINT", -32768.51),
            ("CLNG", 2_147_483_647.5),
            ("CLNG", -2_147_483_648.51),
        ] {
            assert!(matches!(
                registry.call(name, &[Value::Numeric(number)]),
                Err(RuntimeError::Overflow { .. })
            ));
        }
        for name in ["CINT", "CLNG"] {
            for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(matches!(
                    registry.call(name, &[Value::Numeric(number)]),
                    Err(RuntimeError::Overflow { .. })
                ));
            }
            assert!(registry.call(name, &[Value::Int64(i64::MAX)]).is_err());
            assert!(registry.call(name, &[Value::Str("1".into())]).is_err());
        }
        assert!(matches!(
            call_native("CINT", &[Value::Bool(true)]),
            Value::Integer(1)
        ));
        assert!(matches!(
            call_native("CLNG", &[Value::Bool(false)]),
            Value::Long(0)
        ));
        assert!(
            matches!(call_native("CSNG", &[Value::Numeric(0.1)]), Value::Single(n) if n == 0.1_f32)
        );
        assert!(
            matches!(call_native("CSNG", &[Value::Int64(i64::MAX)]), Value::Single(n) if n == i64::MAX as f32)
        );
        assert!(matches!(
            call_native("CSNG", &[Value::Bool(true)]),
            Value::Single(1.0)
        ));
        assert!(matches!(
            registry.call("CSNG", &[Value::Numeric(f64::MAX)]),
            Err(RuntimeError::Overflow { .. })
        ));
        assert!(
            matches!(call_native("CDBL", &[Value::Single(0.1)]), Value::Numeric(n) if n == f64::from(0.1_f32))
        );
    }

    #[test]
    fn fixed_width_math_and_packed_functions_preserve_value_types() {
        for (value, expected) in [
            (Value::Integer(-123), Value::Integer(123)),
            (Value::Long(-123456), Value::Long(123456)),
            (Value::Single(-0.1), Value::Single(0.1)),
        ] {
            let result = call_native("ABS", &[value]);
            assert_eq!(result.get_type(), expected.get_type());
            assert_eq!(result, expected);
        }
        let registry = BuiltinRegistry::new();
        for value in [Value::Integer(i16::MIN), Value::Long(i32::MIN)] {
            assert!(matches!(
                registry.call("ABS", &[value]),
                Err(RuntimeError::Overflow { .. })
            ));
        }
        assert!(matches!(
            call_native("INT", &[Value::Single(-1.5)]),
            Value::Single(-2.0)
        ));
        assert!(matches!(
            call_native("FIX", &[Value::Single(-1.5)]),
            Value::Single(-1.0)
        ));
        assert_eq!(
            call_native("STR$", &[Value::Single(0.1)]),
            Value::Str("0.1".into())
        );
        for (pack, unpack, value) in [
            ("MKI$", "CVI", Value::Integer(i16::MIN)),
            ("MKL$", "CVL", Value::Long(i32::MAX)),
            ("MKS$", "CVS", Value::Single(0.1)),
        ] {
            let decoded = call_native(unpack, &[call_native(pack, &[value.clone()])]);
            assert_eq!(decoded.get_type(), value.get_type());
            assert_eq!(decoded, value);
        }
        for pack in ["MKI$", "MKL$"] {
            assert!(matches!(
                registry.call(pack, &[Value::Numeric(1.5)]),
                Err(RuntimeError::TypeMismatch { .. })
            ));
            assert!(matches!(
                registry.call(pack, &[Value::Int64(i64::MAX)]),
                Err(RuntimeError::Overflow { .. })
            ));
        }
        assert!(matches!(
            registry.call("MKS$", &[Value::Numeric(f64::MAX)]),
            Err(RuntimeError::Overflow { .. })
        ));
        for name in ["MIN", "MAX"] {
            assert!(
                matches!(call_native(name, &[Value::Integer(1), Value::Single(f32::NAN)]), Value::Numeric(n) if n.is_nan())
            );
        }
        assert!(matches!(
            call_native(
                "MAX",
                &[Value::Single(16_777_216.0), Value::Long(16_777_217)]
            ),
            Value::Long(16_777_217)
        ));
    }

    #[test]
    fn integer_math_and_formatting_do_not_round_through_floating_point() {
        let large = 9_007_199_254_740_993_i64;
        assert_eq!(
            call_native("ABS", &[Value::Int64(-large)]),
            Value::Int64(large)
        );
        assert!(
            BuiltinRegistry::new()
                .call("ABS", &[Value::Int64(i64::MIN)])
                .is_err()
        );
        assert_eq!(call_native("ABS", &[Value::Byte(255)]), Value::Byte(255));
        assert_eq!(
            call_native("SGN", &[Value::Int64(-large)]),
            Value::Int64(-1)
        );
        assert_eq!(
            call_native("HEX$", &[Value::Int64(large)]),
            Value::Str("20000000000001".into())
        );
        assert_eq!(
            call_native("OCT$", &[Value::Int64(large)]),
            Value::Str(format!("{large:o}"))
        );
        assert_eq!(
            call_native("HEX$", &[Value::Int64(-1)]),
            Value::Str("FFFFFFFFFFFFFFFF".into())
        );
        let values = [Value::Numeric(9_007_199_254_740_992.0), Value::Int64(large)];
        assert_eq!(call_native("MIN", &values), values[0]);
        assert_eq!(call_native("MAX", &values), values[1]);
        assert_eq!(
            call_native(
                "MAX",
                &[
                    Value::Int64(i64::MAX),
                    Value::Numeric(9_223_372_036_854_775_808.0)
                ]
            ),
            Value::Numeric(9_223_372_036_854_775_808.0)
        );
    }

    #[test]
    fn binary_buffers_round_trip_all_byte_values() {
        let args: Vec<_> = (0..=255).map(Value::Byte).collect();
        let bytes = call_native("BYTES", &args);
        assert_eq!(bytes, Value::Bytes((0..=255).collect()));
        assert_eq!(call_native("BYTES", &[]), Value::Bytes(vec![]));
        assert_eq!(
            call_native("BYTELEN", &[bytes.clone()]),
            Value::Numeric(256.0)
        );
        assert_eq!(call_native("LEN", &[bytes.clone()]), Value::Numeric(256.0));
        let hex = call_native("HEXENCODE$", &[bytes.clone()]);
        assert_eq!(call_native("HEXDECODE", &[hex]), bytes);
        assert_eq!(
            call_native("HEXDECODE", &[Value::Str("00aBfF".into())]),
            Value::Bytes(vec![0, 171, 255])
        );
        assert_eq!(
            call_native("HEXDECODE", &[Value::Str(String::new())]),
            Value::Bytes(vec![])
        );
        for (index, expected) in [(1, 0), (128, 127), (256, 255)] {
            assert_eq!(
                call_native("BYTEAT", &[bytes.clone(), Value::Int64(index)]),
                Value::Byte(expected)
            );
        }
        let changed = call_native(
            "BYTESET",
            &[bytes.clone(), Value::Numeric(1.0), Value::Byte(255)],
        );
        assert_eq!(
            call_native("BYTEAT", &[changed, Value::Numeric(1.0)]),
            Value::Byte(255)
        );
        assert_eq!(
            call_native("BYTEAT", &[bytes.clone(), Value::Numeric(1.0)]),
            Value::Byte(0)
        );
        assert_eq!(
            call_native("BYTESLICE", &[bytes.clone(), Value::Numeric(255.0)]),
            Value::Bytes(vec![254, 255])
        );
        assert_eq!(
            call_native(
                "BYTESLICE",
                &[bytes.clone(), Value::Numeric(255.0), Value::Int64(i64::MAX)]
            ),
            Value::Bytes(vec![254, 255])
        );
        assert_eq!(
            call_native("BYTESLICE", &[bytes.clone(), Value::Numeric(257.0)]),
            Value::Bytes(vec![])
        );
        assert_eq!(
            call_native(
                "BYTESLICE",
                &[bytes, Value::Numeric(1.0), Value::Numeric(0.0)]
            ),
            Value::Bytes(vec![])
        );
    }

    #[test]
    fn binary_buffers_validate_types_indexes_counts_and_encoding() {
        let registry = BuiltinRegistry::new();
        for index in [
            Value::Numeric(0.0),
            Value::Numeric(-1.0),
            Value::Numeric(2.0),
            Value::Numeric(1.5),
            Value::Int64(i64::MIN),
        ] {
            assert!(
                registry
                    .call("BYTEAT", &[Value::Bytes(vec![0]), index])
                    .is_err()
            );
        }
        for invalid in ["0", "GG", " 00", "00 ", "é", "0x00"] {
            assert!(
                registry
                    .call("HEXDECODE", &[Value::Str(invalid.into())])
                    .is_err()
            );
        }
        for bytes in [
            vec![255],
            vec![0xc0, 0x80],
            vec![0xed, 0xa0, 0x80],
            vec![0xe2, 0x82],
        ] {
            assert!(
                registry
                    .call("UTF8DECODE$", &[Value::Bytes(bytes)])
                    .is_err()
            );
        }
        for text in ["", "\0", "Rice 🍚\0é", "a\u{301}"] {
            let value = Value::Str(text.into());
            let encoded = call_native("UTF8ENCODE", &[value.clone()]);
            assert_eq!(encoded, Value::Bytes(text.as_bytes().to_vec()));
            assert_eq!(call_native("UTF8DECODE$", &[encoded]), value);
        }
        for count in [
            Value::Numeric(-1.0),
            Value::Numeric(1.5),
            Value::Numeric(f64::INFINITY),
        ] {
            assert!(registry.call("BYTEBUFFER", &[count.clone()]).is_err());
            assert!(
                registry
                    .call(
                        "BYTESLICE",
                        &[Value::Bytes(vec![]), Value::Numeric(1.0), count]
                    )
                    .is_err()
            );
        }
        for name in ["BYTELEN", "HEXENCODE$", "UTF8DECODE$"] {
            assert!(registry.call(name, &[Value::Str(String::new())]).is_err());
        }
        assert!(registry.call("BYTES", &[Value::Numeric(256.0)]).is_err());
        assert!(
            registry
                .call(
                    "BYTESET",
                    &[
                        Value::Bytes(vec![0]),
                        Value::Numeric(1.0),
                        Value::Numeric(0.5)
                    ]
                )
                .is_err()
        );
        assert_eq!(
            call_native("BYTEBUFFER", &[Value::Numeric(3.0)]),
            Value::Bytes(vec![0, 0, 0])
        );
        assert_eq!(
            call_native("BYTEBUFFER", &[Value::Numeric(2.0), Value::Byte(255)]),
            Value::Bytes(vec![255, 255])
        );
        assert_eq!(
            call_native("BYTEBUFFER", &[Value::Numeric(0.0)]),
            Value::Bytes(vec![])
        );
        assert!(
            registry
                .call("BYTEBUFFER", &[Value::Int64(i64::MAX)])
                .is_err()
        );
    }

    #[test]
    fn strcomp_orders_strings_and_supports_unicode_lowercase_mode() {
        let registry = BuiltinRegistry::new();
        for (left, right, mode, expected) in [
            ("", "", None, 0.0),
            ("", "a", None, -1.0),
            ("a", "", None, 1.0),
            ("a", "aa", None, -1.0),
            ("Z", "a", None, -1.0),
            ("é", "z", Some(0.0), 1.0),
            ("Rice", "rice", Some(0.0), -1.0),
            ("Rice", "rice", Some(1.0), 0.0),
            ("É", "é", Some(1.0), 0.0),
            ("İ", "i\u{307}", Some(1.0), 0.0),
            ("ß", "SS", Some(1.0), 1.0),
            ("é", "e\u{301}", Some(1.0), 1.0),
            ("Z", "a", Some(1.0), 1.0),
        ] {
            let mut args = vec![Value::Str(left.into()), Value::Str(right.into())];
            if let Some(mode) = mode {
                args.push(Value::Numeric(mode));
            }
            assert_eq!(
                registry.call("strcomp", &args).unwrap(),
                Some(Value::Numeric(expected)),
                "{left:?}, {right:?}, {mode:?}",
            );
        }
    }

    #[test]
    fn strcomp_rejects_invalid_modes_types_and_arities() {
        let registry = BuiltinRegistry::new();
        let text = Value::Str("same".into());
        for mode in [-1.0, 0.5, 1.5, 2.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                registry.call(
                    "STRCOMP",
                    &[text.clone(), text.clone(), Value::Numeric(mode)]
                ),
                Err(RuntimeError::IllegalFunctionCall { .. }),
            ));
        }
        for args in [
            vec![Value::Numeric(1.0), text.clone()],
            vec![text.clone(), Value::Numeric(1.0)],
            vec![text.clone(), text.clone(), Value::Str("1".into())],
            vec![
                Value::Record {
                    type_name: "T".into(),
                    fields: HashMap::new(),
                },
                text.clone(),
            ],
        ] {
            assert!(matches!(
                registry.call("STRCOMP", &args),
                Err(RuntimeError::TypeMismatch { .. })
            ));
        }
        for count in [0, 1, 4] {
            assert!(matches!(
                registry.call("STRCOMP", &vec![text.clone(); count]),
                Err(RuntimeError::ArityMismatch { .. }),
            ));
        }
    }

    #[test]
    fn standalone_command_has_no_host_arguments_and_checks_arity() {
        let registry = BuiltinRegistry::new();
        for name in ["COMMAND$", "COMMAND"] {
            assert!(registry.exists(name));
            assert_eq!(
                registry.call(name, &[]).unwrap(),
                Some(Value::Str(String::new()))
            );
            assert!(matches!(
                registry.call(name, &[Value::Numeric(1.0)]),
                Err(RuntimeError::ArityMismatch {
                    expected: 0,
                    got: 1
                })
            ));
        }
    }

    #[test]
    fn minmax_accepts_one_or_more_numeric_arguments() {
        let registry = BuiltinRegistry::new();
        for name in ["MIN", "MAX"] {
            assert_eq!(
                registry.call(name, &[Value::Numeric(-2.5)]).unwrap(),
                Some(Value::Numeric(-2.5)),
            );
        }
        let values = [6.0, -8.0, 2.5, -8.0, 0.0].map(Value::Numeric);
        assert_eq!(
            registry.call("min", &values).unwrap(),
            Some(Value::Numeric(-8.0)),
        );
        assert_eq!(
            registry.call("max", &values).unwrap(),
            Some(Value::Numeric(6.0)),
        );
    }

    #[test]
    fn minmax_rejects_empty_and_nonnumeric_arguments() {
        let registry = BuiltinRegistry::new();
        for name in ["MIN", "MAX"] {
            assert!(matches!(
                registry.call(name, &[]),
                Err(RuntimeError::ArityMismatch {
                    expected: 1,
                    got: 0
                }),
            ));
            for invalid in [
                Value::Str("2".into()),
                Value::Record {
                    type_name: "POINT".into(),
                    fields: HashMap::new(),
                },
            ] {
                assert!(matches!(
                    registry.call(name, std::slice::from_ref(&invalid)),
                    Err(RuntimeError::TypeMismatch { .. }),
                ));
                assert!(matches!(
                    registry.call(name, &[Value::Numeric(f64::NAN), invalid]),
                    Err(RuntimeError::TypeMismatch { .. }),
                ));
            }
        }
    }

    #[test]
    fn minmax_propagates_nan_and_orders_infinities() {
        let registry = BuiltinRegistry::new();
        for name in ["MIN", "MAX"] {
            for nan_index in 0..3 {
                let mut values = [
                    Value::Numeric(2.0),
                    Value::Numeric(-3.0),
                    Value::Numeric(8.0),
                ];
                values[nan_index] = Value::Numeric(f64::NAN);
                assert!(matches!(
                    registry.call(name, &values).unwrap(),
                    Some(Value::Numeric(n)) if n.is_nan(),
                ));
            }
        }
        let values = [f64::INFINITY, 0.0, f64::NEG_INFINITY].map(Value::Numeric);
        assert_eq!(
            registry.call("MIN", &values).unwrap(),
            Some(Value::Numeric(f64::NEG_INFINITY)),
        );
        assert_eq!(
            registry.call("MAX", &values).unwrap(),
            Some(Value::Numeric(f64::INFINITY)),
        );
    }

    #[test]
    fn decimal_rounding_does_not_overflow_finite_values() {
        for function in [builtin_round as BuiltinFn, builtin_truncate] {
            for n in [f64::MAX, -f64::MAX, 1e200] {
                assert_eq!(
                    function(&[Value::Numeric(n), Value::Numeric(308.0)]).unwrap(),
                    Value::Numeric(n),
                );
            }
        }
    }

    #[test]
    fn asc_round_trips_every_basic_byte() {
        let registry = BuiltinRegistry::new();
        for code in 0..=255 {
            let character = registry
                .call("CHR$", &[Value::Numeric(code as f64)])
                .unwrap()
                .unwrap();
            assert_eq!(
                registry.call("ASC", &[character]).unwrap(),
                Some(Value::Numeric(code as f64)),
            );
        }
        assert!(matches!(
            registry.call("ASC", &[Value::Str(String::new())]),
            Err(RuntimeError::IllegalFunctionCall { .. })
        ));
    }

    #[test]
    fn instr_validates_start_before_subtracting() {
        for start in [i64::MIN as f64, -1.0, 0.0] {
            assert!(matches!(
                builtin_instr(&[
                    Value::Numeric(start),
                    Value::Str("abc".into()),
                    Value::Str("a".into()),
                ]),
                Err(RuntimeError::IllegalFunctionCall { .. })
            ));
        }
    }

    #[test]
    fn instr_respects_character_positions_and_end_of_string() {
        for (start, needle, expected) in [
            (2.0, "abc", 2.0),
            (3.0, "abc", 5.0),
            (2.0, "", 2.0),
            (8.0, "", 0.0),
            (1e15, "", 0.0),
        ] {
            assert_eq!(
                builtin_instr(&[
                    Value::Numeric(start),
                    Value::Str("éabcabc".into()),
                    Value::Str(needle.into()),
                ])
                .unwrap(),
                Value::Numeric(expected),
            );
        }
    }

    #[test]
    fn val_parses_scientific_prefixes_and_stops_at_invalid_characters() {
        for (text, expected) in [
            ("1E2tail", 100.0),
            ("-2.5d+2🙂", -250.0),
            (".5D-1", 0.05),
            ("+12.e+1rest", 120.0),
            ("1.2.3", 1.2),
            ("1..2", 1.0),
            (".3.4", 0.3),
            ("1E", 1.0),
            ("2D+", 2.0),
            ("3e-words", 3.0),
            ("4D+-2", 4.0),
            ("5E.2", 5.0),
            ("6E2D3", 600.0),
            ("  -7.5D1 trailing  ", -75.0),
            ("1 2", 1.0),
            ("", 0.0),
            (".", 0.0),
            ("+.", 0.0),
            ("E2", 0.0),
            ("é123", 0.0),
            ("--2", 0.0),
            ("2é3", 2.0),
        ] {
            assert_eq!(
                builtin_val(&[Value::Str(text.into())]).unwrap(),
                Value::Numeric(expected),
                "{text}"
            );
        }
    }

    #[test]
    fn val_preserves_radix_nonfinite_and_type_rules() {
        for (text, expected) in [
            ("&HFFtail", 255.0),
            ("&O778", 63.0),
            ("-&H10", 0.0),
            ("&HFFFFFFFFFFFFFFFF", 0.0),
            ("&H7FFFFFFFFFFFFFFF", i64::MAX as f64),
            ("&H", 0.0),
            ("inf", f64::INFINITY),
            ("-inf", f64::NEG_INFINITY),
            ("1D999tail", f64::INFINITY),
            ("1E-999tail", 0.0),
        ] {
            assert_eq!(
                builtin_val(&[Value::Str(text.into())]).unwrap(),
                Value::Numeric(expected),
                "{text}"
            );
        }
        assert!(
            builtin_val(&[Value::Str("NaN".into())])
                .unwrap()
                .to_f64()
                .unwrap()
                .is_nan()
        );
        assert!(
            builtin_val(&[Value::Str("-0D1tail".into())])
                .unwrap()
                .to_f64()
                .unwrap()
                .is_sign_negative()
        );
        let registry = BuiltinRegistry::new();
        for args in [vec![], vec![Value::Str("1".into()), Value::Str("2".into())]] {
            assert!(matches!(
                registry.call("VAL", &args),
                Err(RuntimeError::ArityMismatch { .. })
            ));
        }
        for value in [
            Value::Numeric(1.0),
            Value::Record {
                type_name: "R".into(),
                fields: HashMap::new(),
            },
        ] {
            assert!(matches!(
                registry.call("VAL", &[value]),
                Err(RuntimeError::TypeMismatch { .. })
            ));
        }
    }

    #[test]
    fn environment_entries_are_sorted_and_overrides_replace_host_values() {
        let host = [
            ("Z_LAST", "last"),
            ("A_FIRST", "first"),
            ("M_MIDDLE", "old"),
        ]
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));
        let overrides = HashMap::from([
            ("M_MIDDLE".into(), "new=value".into()),
            ("B_ADDED".into(), String::new()),
        ]);
        assert_eq!(
            ordered_environment_entries(host, &overrides)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![
                ("A_FIRST".into(), "first".into()),
                ("B_ADDED".into(), String::new()),
                ("M_MIDDLE".into(), "new=value".into()),
                ("Z_LAST".into(), "last".into()),
            ]
        );
        let host = [(OsString::from("mixed"), OsString::from("lower"))];
        let overrides = HashMap::from([("MIXED".into(), "upper".into())]);
        let entries = ordered_environment_entries(host, &overrides);
        assert_eq!(entries.len(), if cfg!(windows) { 1 } else { 2 });
        assert_eq!(entries["MIXED"], "upper");
    }

    #[cfg(unix)]
    #[test]
    fn environment_entries_skip_non_unicode_names_and_values() {
        use std::os::unix::ffi::OsStringExt;
        let invalid = OsString::from_vec(vec![255]);
        let entries = ordered_environment_entries(
            [
                (invalid.clone(), OsString::from("value")),
                (OsString::from("INVALID_VALUE"), invalid),
                (OsString::from("VALID"), OsString::from("é🙂")),
            ],
            &HashMap::new(),
        );
        assert_eq!(entries, BTreeMap::from([("VALID".into(), "é🙂".into())]));
    }

    #[test]
    fn environment_index_checks_boundaries_and_registry_aliases() {
        let registry = BuiltinRegistry::new();
        for name in ["ENVIRON$", "ENVIRON"] {
            for index in [0.0, -1.0, 0.9, i64::MIN as f64] {
                assert!(matches!(
                    registry.call(name, &[Value::Numeric(index)]),
                    Err(RuntimeError::IllegalFunctionCall { .. })
                ));
            }
            for index in [
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
                9_223_372_036_854_775_808.0,
            ] {
                assert!(matches!(
                    registry.call(name, &[Value::Numeric(index)]),
                    Err(RuntimeError::Overflow { .. })
                ));
            }
            assert_eq!(
                registry.call(name, &[Value::Numeric(1e15)]).unwrap(),
                Some(Value::Str(String::new()))
            );
            assert!(matches!(
                registry.call(name, &[]),
                Err(RuntimeError::ArityMismatch { .. })
            ));
            assert!(matches!(
                registry.call(name, &[Value::Numeric(1.0), Value::Numeric(2.0)]),
                Err(RuntimeError::ArityMismatch { .. })
            ));
            assert!(matches!(
                registry.call(
                    name,
                    &[Value::Record {
                        type_name: "R".into(),
                        fields: HashMap::new()
                    }]
                ),
                Err(RuntimeError::TypeMismatch { .. })
            ));
        }
    }

    #[test]
    fn replace_uses_unicode_suffixes_and_nonoverlapping_matches() {
        for (source, find, replacement, start, count, expected) in [
            ("banana", "na", "XYZ", 1.0, -1.0, "baXYZXYZ"),
            ("aaaaa", "aa", "b", 1.0, -1.0, "bba"),
            ("aba", "a", "aa", 1.0, -1.0, "aabaa"),
            ("é🙂é🙂", "é", "X", 2.0, -1.0, "🙂X🙂"),
            ("é🙂é🙂", "🙂", "", 1.0, -1.0, "éé"),
            ("prefix abc abc", "abc", "X", 8.0, 1.0, "X abc"),
            ("abc", "", "X", 2.0, -1.0, "bc"),
            ("abc", "b", "X", 2.0, 0.0, "bc"),
            ("abc", "b", "X", 4.0, -1.0, ""),
            ("", "a", "X", 1.0, -1.0, ""),
            ("aaaa", "a", "b", 2.9, 1.9, "baa"),
            ("aaaa", "a", "b", 1.0, 1e15, "bbbb"),
            ("abc", "a", "b", 1e15, -1.0, ""),
        ] {
            assert_eq!(
                builtin_replace(&[
                    Value::Str(source.into()),
                    Value::Str(find.into()),
                    Value::Str(replacement.into()),
                    Value::Numeric(start),
                    Value::Numeric(count),
                ])
                .unwrap(),
                Value::Str(expected.into())
            );
        }
        let registry = BuiltinRegistry::new();
        for name in ["REPLACE$", "REPLACE"] {
            assert!(registry.exists(name));
            assert_eq!(
                registry
                    .call(
                        name,
                        &[
                            Value::Str("aba".into()),
                            Value::Str("a".into()),
                            Value::Str("x".into()),
                        ]
                    )
                    .unwrap(),
                Some(Value::Str("xbx".into()))
            );
        }
    }

    #[test]
    fn instrrev_limits_match_end_and_handles_empty_inputs() {
        for (source, needle, start, expected) in [
            ("banana", "ana", -1.0, 4.0),
            ("banana", "ana", 5.0, 2.0),
            ("banana", "ana", 3.0, 0.0),
            ("é🙂é🙂", "🙂", -1.0, 4.0),
            ("é🙂é🙂", "é🙂", 3.0, 1.0),
            ("é🙂é🙂", "🙂", 3.0, 2.0),
            ("aaa", "aa", -1.0, 2.0),
            ("abc", "", -1.0, 3.0),
            ("abc", "", 2.0, 2.0),
            ("abc", "", 4.0, 0.0),
            ("", "", -1.0, 0.0),
            ("", "x", 1.0, 0.0),
            ("abc", "A", -1.0, 0.0),
            ("abc", "a", 1e15, 0.0),
            ("aba", "a", 2.9, 1.0),
            ("aba", "a", -1.9, 3.0),
        ] {
            assert_eq!(
                builtin_instrrev(&[
                    Value::Str(source.into()),
                    Value::Str(needle.into()),
                    Value::Numeric(start),
                ])
                .unwrap(),
                Value::Numeric(expected)
            );
        }
        assert_eq!(
            builtin_instrrev(&[Value::Str("aba".into()), Value::Str("a".into())]).unwrap(),
            Value::Numeric(3.0)
        );
    }

    #[test]
    fn replacement_and_reverse_search_validate_noop_arguments_and_arity() {
        let registry = BuiltinRegistry::new();
        for (name, valid, arities) in [
            (
                "REPLACE$",
                vec![
                    Value::Str("".into()),
                    Value::Str("".into()),
                    Value::Str("".into()),
                    Value::Numeric(1.0),
                    Value::Numeric(0.0),
                ],
                vec![0, 1, 2, 6],
            ),
            (
                "INSTRREV",
                vec![
                    Value::Str("".into()),
                    Value::Str("".into()),
                    Value::Numeric(1.0),
                ],
                vec![0, 1, 4],
            ),
        ] {
            for arity in arities {
                assert!(matches!(
                    registry.call(name, &vec![Value::Numeric(1.0); arity]),
                    Err(RuntimeError::ArityMismatch { .. })
                ));
            }
            for index in 0..valid.len() {
                let mut args = valid.clone();
                args[index] = if args[index].is_numeric() {
                    Value::Str("1".into())
                } else {
                    Value::Numeric(1.0)
                };
                assert!(
                    matches!(
                        registry.call(name, &args),
                        Err(RuntimeError::TypeMismatch { .. })
                    ),
                    "{name} argument {index}"
                );
                args[index] = Value::Record {
                    type_name: "R".into(),
                    fields: HashMap::new(),
                };
                assert!(matches!(
                    registry.call(name, &args),
                    Err(RuntimeError::TypeMismatch { .. })
                ));
            }
            let position = if name == "REPLACE$" { 3 } else { 2 };
            for bad in [0.0, -2.0, i64::MIN as f64] {
                let mut args = valid.clone();
                args[position] = Value::Numeric(bad);
                assert!(matches!(
                    registry.call(name, &args),
                    Err(RuntimeError::IllegalFunctionCall { .. })
                ));
            }
            for bad in [
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
                9_223_372_036_854_775_808.0,
            ] {
                let mut args = valid.clone();
                args[position] = Value::Numeric(bad);
                assert!(matches!(
                    registry.call(name, &args),
                    Err(RuntimeError::Overflow { .. })
                ));
                if name == "REPLACE$" {
                    args[position] = Value::Numeric(1.0);
                    args[4] = Value::Numeric(bad);
                    assert!(matches!(
                        registry.call(name, &args),
                        Err(RuntimeError::Overflow { .. })
                    ));
                }
            }
        }
        for count in [-2.0, i64::MIN as f64] {
            assert!(matches!(
                builtin_replace(&[
                    Value::Str("".into()),
                    Value::Str("".into()),
                    Value::Str("".into()),
                    Value::Numeric(1.0),
                    Value::Numeric(count)
                ]),
                Err(RuntimeError::IllegalFunctionCall { .. })
            ));
        }
    }

    #[test]
    fn negative_decimal_places_round_and_truncate_without_changing_positive_precision() {
        for (input, places, rounded, truncated) in [
            (149.0, -1.0, 150.0, 140.0),
            (150.0, -2.0, 200.0, 100.0),
            (-150.0, -2.0, -200.0, -100.0),
            (1499.0, -2.0, 1500.0, 1400.0),
            (-1499.0, -2.0, -1500.0, -1400.0),
            (1e308, -308.0, 1e308, 1e308),
            (1e23, -23.0, 1e23, 1e23),
            (149.0, -1.9, 150.0, 140.0),
            (1.25, 1.0, 1.3, 1.2),
            (1.25, -0.9, 1.0, 1.0),
        ] {
            let args = [Value::Numeric(input), Value::Numeric(places)];
            assert_eq!(builtin_round(&args).unwrap(), Value::Numeric(rounded));
            assert_eq!(builtin_truncate(&args).unwrap(), Value::Numeric(truncated));
        }
        for function in [builtin_round as BuiltinFn, builtin_truncate] {
            for input in [-0.0, -f64::MIN_POSITIVE, -f64::from_bits(1)] {
                let result = function(&[Value::Numeric(input), Value::Numeric(-308.0)])
                    .unwrap()
                    .to_f64()
                    .unwrap();
                assert_eq!(result, 0.0);
                assert!(result.is_sign_negative());
            }
            for input in [f64::INFINITY, f64::NEG_INFINITY] {
                assert_eq!(
                    function(&[Value::Numeric(input), Value::Numeric(-308.0)]).unwrap(),
                    Value::Numeric(input)
                );
            }
            assert!(
                function(&[Value::Numeric(f64::NAN), Value::Numeric(-1.0)])
                    .unwrap()
                    .to_f64()
                    .unwrap()
                    .is_nan()
            );
        }
    }

    #[test]
    fn negative_decimal_places_check_range_arity_types_and_rounding_overflow() {
        let registry = BuiltinRegistry::new();
        for name in ["ROUND", "TRUNCATE"] {
            for places in [-309.0, 309.0, i64::MIN as f64] {
                assert!(matches!(
                    registry.call(name, &[Value::Numeric(1.0), Value::Numeric(places)]),
                    Err(RuntimeError::IllegalFunctionCall { .. })
                ));
            }
            for places in [f64::NAN, f64::INFINITY, 9_223_372_036_854_775_808.0] {
                assert!(matches!(
                    registry.call(name, &[Value::Numeric(1.0), Value::Numeric(places)]),
                    Err(RuntimeError::Overflow { .. })
                ));
            }
            for args in [vec![], vec![Value::Numeric(1.0); 3]] {
                assert!(matches!(
                    registry.call(name, &args),
                    Err(RuntimeError::ArityMismatch { .. })
                ));
            }
            for args in [
                vec![Value::Str("1".into()), Value::Numeric(-1.0)],
                vec![Value::Numeric(1.0), Value::Str("-1".into())],
            ] {
                assert!(matches!(
                    registry.call(name, &args),
                    Err(RuntimeError::TypeMismatch { .. })
                ));
            }
            assert!(matches!(
                registry.call(name, &[Value::Numeric(f64::NAN), Value::Numeric(-309.0)]),
                Err(RuntimeError::IllegalFunctionCall { .. })
            ));
        }
        for number in [f64::MAX, -f64::MAX] {
            assert!(matches!(
                builtin_round(&[Value::Numeric(number), Value::Numeric(-308.0)]),
                Err(RuntimeError::Overflow { .. })
            ));
            assert_eq!(
                builtin_truncate(&[Value::Numeric(number), Value::Numeric(-308.0)]).unwrap(),
                Value::Numeric(1e308_f64.copysign(number))
            );
        }
    }
}
