use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;

use crate::error::RuntimeError;
use crate::value::{Value, basic_string_to_bytes, bytes_to_basic_string};

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
        // ANSI math additions
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

// Math builtins

fn builtin_abs(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(Value::Numeric(n.abs()))
}

fn builtin_int(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(Value::Numeric(n.floor()))
}

fn builtin_fix(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(Value::Numeric(n.trunc()))
}

/// Round half-to-even (banker's rounding), matching QuickBasic's CINT/CLNG.
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
    let n = args[0].to_f64()?;
    if !(-32768.0..=32767.0).contains(&n) {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "CINT: value out of integer range (-32768..32767)".into(),
        });
    }
    Ok(Value::Numeric(round_half_even(n)))
}

fn builtin_clng(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    if !(-2_147_483_648.0..=2_147_483_647.0).contains(&n) {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "CLNG: value out of long range".into(),
        });
    }
    Ok(Value::Numeric(round_half_even(n)))
}

fn builtin_csng(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(Value::Numeric(n as f32 as f64))
}

fn builtin_cdbl(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(Value::Numeric(n))
}

fn builtin_sgn(args: &[Value]) -> Result<Value, RuntimeError> {
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

fn builtin_min(args: &[Value]) -> Result<Value, RuntimeError> {
    numeric_extremum(args, f64::min)
}

fn builtin_max(args: &[Value]) -> Result<Value, RuntimeError> {
    numeric_extremum(args, f64::max)
}

fn numeric_extremum(args: &[Value], select: fn(f64, f64) -> f64) -> Result<Value, RuntimeError> {
    let mut result = args
        .first()
        .ok_or(RuntimeError::ArityMismatch {
            expected: 1,
            got: 0,
        })?
        .to_f64()?;
    for arg in &args[1..] {
        let value = arg.to_f64()?;
        // Propagate NaN, but continue validating the remaining arguments.
        result = if result.is_nan() || value.is_nan() {
            f64::NAN
        } else {
            select(result, value)
        };
    }
    Ok(Value::Numeric(result))
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
        _ => Err(RuntimeError::TypeMismatch {
            msg: "LEN expects a string".into(),
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
    // QuickBasic hex (&H) and octal (&O) literals
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
    let n = args[0].to_f64()?;
    let i = n as i64;
    if i < 0 {
        Ok(Value::Str(format!("{:X}", i as u64)))
    } else {
        Ok(Value::Str(format!("{:X}", i)))
    }
}

fn builtin_oct(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    let i = n as i64;
    if i < 0 {
        Ok(Value::Str(format!("{:o}", i as u64)))
    } else {
        Ok(Value::Str(format!("{:o}", i)))
    }
}

fn builtin_mki(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()? as i16;
    Ok(Value::Str(bytes_to_basic_string(&n.to_le_bytes())))
}

fn builtin_mkl(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()? as i32;
    Ok(Value::Str(bytes_to_basic_string(&n.to_le_bytes())))
}

fn builtin_mks(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()? as f32;
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
    Ok(Value::Numeric(
        i16::from_le_bytes([bytes[0], bytes[1]]) as f64
    ))
}

fn builtin_cvl(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = require_packed_bytes(args, "CVL", 4)?;
    Ok(Value::Numeric(
        i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64,
    ))
}

fn builtin_cvs(args: &[Value]) -> Result<Value, RuntimeError> {
    let bytes = require_packed_bytes(args, "CVS", 4)?;
    Ok(Value::Numeric(
        f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64,
    ))
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
}
