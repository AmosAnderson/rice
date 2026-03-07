use std::collections::HashMap;

use crate::error::RuntimeError;
use crate::value::Value;

pub type BuiltinFn = fn(&[Value]) -> Result<Value, RuntimeError>;

pub struct BuiltinRegistry {
    functions: HashMap<String, (BuiltinFn, usize)>, // (function, expected_args) — 0 means variadic
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
        // RND is handled as a stateful function in the interpreter
        reg.register("CINT", builtin_cint, 1);
        reg.register("CLNG", builtin_clng, 1);
        reg.register("CSNG", builtin_csng, 1);
        reg.register("CDBL", builtin_cdbl, 1);

        // String
        reg.register("LEN", builtin_len, 1);
        reg.register("LEFT$", builtin_left, 2);
        reg.register("RIGHT$", builtin_right, 2);
        reg.register("MID$", builtin_mid, 0); // 2 or 3 args
        reg.register("INSTR", builtin_instr, 0); // 2 or 3 args
        reg.register("UCASE$", builtin_ucase, 1);
        reg.register("LCASE$", builtin_lcase, 1);
        reg.register("LTRIM$", builtin_ltrim, 1);
        reg.register("RTRIM$", builtin_rtrim, 1);
        reg.register("SPACE$", builtin_space, 1);
        reg.register("STRING$", builtin_string_fn, 2);
        reg.register("CHR$", builtin_chr, 1);
        reg.register("ASC", builtin_asc, 1);
        reg.register("STR$", builtin_str, 1);
        reg.register("VAL", builtin_val, 1);
        reg.register("HEX$", builtin_hex, 1);
        reg.register("OCT$", builtin_oct, 1);

        // Misc
        reg.register("LBOUND", builtin_stub, 0);
        reg.register("UBOUND", builtin_stub, 0);
        reg.register("TIMER", builtin_timer, 0);
        reg.register("DATE$", builtin_date, 0);
        reg.register("TIME$", builtin_time, 0);
        reg.register("COMMAND$", builtin_stub, 0);
        reg.register("ENVIRON$", builtin_environ, 1);

        // Binary conversion
        reg.register("MKI$", builtin_mki, 1);
        reg.register("MKL$", builtin_mkl, 1);
        reg.register("MKS$", builtin_mks, 1);
        reg.register("MKD$", builtin_mkd, 1);
        reg.register("CVI", builtin_cvi, 1);
        reg.register("CVL", builtin_cvl, 1);
        reg.register("CVS", builtin_cvs, 1);
        reg.register("CVD", builtin_cvd, 1);

        reg
    }

    fn register(&mut self, name: &str, func: BuiltinFn, args: usize) {
        self.functions.insert(name.to_string(), (func, args));
    }

    pub fn call(&self, name: &str, args: &[Value]) -> Result<Option<Value>, RuntimeError> {
        let name_upper = name.to_uppercase();
        // Try name as-is, then with $
        let func_info = self.functions.get(&name_upper)
            .or_else(|| self.functions.get(&format!("{}$", name_upper)));
        if let Some((func, expected)) = func_info {
            if *expected > 0 && args.len() != *expected {
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

fn preserve_type(result: f64, original: &Value) -> Value {
    match original {
        Value::Integer(_) => Value::Integer(result as i64),
        Value::Long(_) => Value::Long(result as i64),
        Value::Single(_) => Value::Single(result),
        _ => Value::Double(result),
    }
}

fn builtin_abs(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(preserve_type(n.abs(), &args[0]))
}

fn builtin_int(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(preserve_type(n.floor(), &args[0]))
}

fn builtin_fix(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(preserve_type(n.trunc(), &args[0]))
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
    Ok(Value::Integer(s))
}

fn builtin_sqr(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    if n < 0.0 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "SQR of negative number".into(),
        });
    }
    Ok(Value::Double(n.sqrt()))
}

fn builtin_sin(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Double(args[0].to_f64()?.sin()))
}

fn builtin_cos(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Double(args[0].to_f64()?.cos()))
}

fn builtin_tan(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Double(args[0].to_f64()?.tan()))
}

fn builtin_atn(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Double(args[0].to_f64()?.atan()))
}

fn builtin_exp(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Double(args[0].to_f64()?.exp()))
}

fn builtin_log(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    if n <= 0.0 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "LOG of non-positive number".into(),
        });
    }
    Ok(Value::Double(n.ln()))
}

fn builtin_cint(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?.round();
    if n < -32768.0 || n > 32767.0 {
        return Err(RuntimeError::Overflow { msg: "CINT overflow".into() });
    }
    Ok(Value::Integer(n as i64))
}

fn builtin_clng(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?.round();
    if n < -2_147_483_648.0 || n > 2_147_483_647.0 {
        return Err(RuntimeError::Overflow { msg: "CLNG overflow".into() });
    }
    Ok(Value::Long(n as i64))
}

fn builtin_csng(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Single(args[0].to_f64()?))
}

fn builtin_cdbl(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Double(args[0].to_f64()?))
}

// String builtins

fn builtin_len(args: &[Value]) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Str(s) => Ok(Value::Integer(s.chars().count() as i64)),
        _ => Err(RuntimeError::TypeMismatch {
            msg: "LEN expects a string".into(),
        }),
    }
}

fn builtin_left(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let n_raw = args[1].to_i64()?;
    if n_raw < 0 {
        return Err(RuntimeError::IllegalFunctionCall { msg: "LEFT$ count must be non-negative".into() });
    }
    let n = n_raw as usize;
    let result: String = s.chars().take(n).collect();
    Ok(Value::Str(result))
}

fn builtin_right(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let n = args[1].to_i64()? as usize;
    let len = s.chars().count();
    let skip = len.saturating_sub(n);
    let result: String = s.chars().skip(skip).collect();
    Ok(Value::Str(result))
}

fn builtin_mid(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 || args.len() > 3 {
        return Err(RuntimeError::ArityMismatch {
            expected: 2,
            got: args.len(),
        });
    }
    let s = args[0].to_string_val()?;
    let start = (args[1].to_i64()? - 1).max(0) as usize; // 1-based
    if args.len() == 3 {
        let len = args[2].to_i64()? as usize;
        let result: String = s.chars().skip(start).take(len).collect();
        Ok(Value::Str(result))
    } else {
        let result: String = s.chars().skip(start).collect();
        Ok(Value::Str(result))
    }
}

fn builtin_instr(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.len() {
        2 => {
            let haystack = args[0].to_string_val()?;
            let needle = args[1].to_string_val()?;
            let pos = haystack.find(&needle).map(|p| p + 1).unwrap_or(0);
            Ok(Value::Integer(pos as i64))
        }
        3 => {
            let start = (args[0].to_i64()? - 1).max(0) as usize;
            let haystack = args[1].to_string_val()?;
            let needle = args[2].to_string_val()?;
            let pos = haystack[start.min(haystack.len())..]
                .find(&needle)
                .map(|p| p + start + 1)
                .unwrap_or(0);
            Ok(Value::Integer(pos as i64))
        }
        _ => Err(RuntimeError::ArityMismatch {
            expected: 2,
            got: args.len(),
        }),
    }
}

fn builtin_ucase(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Str(args[0].to_string_val()?.to_uppercase()))
}

fn builtin_lcase(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Str(args[0].to_string_val()?.to_lowercase()))
}

fn builtin_ltrim(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Str(args[0].to_string_val()?.trim_start().to_string()))
}

fn builtin_rtrim(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Str(args[0].to_string_val()?.trim_end().to_string()))
}

fn builtin_space(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()?;
    if n < 0 {
        return Err(RuntimeError::IllegalFunctionCall { msg: "SPACE$ argument must be non-negative".into() });
    }
    Ok(Value::Str(" ".repeat(n as usize)))
}

fn builtin_string_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()?;
    if n < 0 {
        return Err(RuntimeError::IllegalFunctionCall { msg: "STRING$ count must be non-negative".into() });
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
    if s.is_empty() {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "ASC of empty string".into(),
        });
    }
    Ok(Value::Integer(s.as_bytes()[0] as i64))
}

fn builtin_str(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    let sign = if n >= 0.0 { " " } else { "" };
    let formatted = if n == n.trunc() && n.abs() < 1e15 {
        format!("{sign}{}", n as i64)
    } else {
        format!("{sign}{n}")
    };
    Ok(Value::Str(formatted))
}

fn builtin_val(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let s = s.trim();
    if s.is_empty() {
        return Ok(Value::Double(0.0));
    }
    // Try integer first, then float
    if let Ok(n) = s.parse::<i64>() {
        return Ok(Value::Double(n as f64));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(Value::Double(n));
    }
    // QBasic returns 0 for non-numeric strings after parsing leading digits
    let mut num_str = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_digit() || ch == '.' || ((ch == '-' || ch == '+') && i == 0) {
            num_str.push(ch);
        } else {
            break;
        }
    }
    if num_str.is_empty() {
        Ok(Value::Double(0.0))
    } else {
        Ok(Value::Double(num_str.parse::<f64>().unwrap_or(0.0)))
    }
}

fn builtin_hex(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()?;
    Ok(Value::Str(format!("{:X}", n)))
}

fn builtin_oct(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()?;
    Ok(Value::Str(format!("{:o}", n)))
}

/// Get local time components (hours, minutes, seconds, millis) from system time.
/// Uses platform-specific APIs to get local time without external dependencies.
fn local_time_parts() -> (u64, u64, u64, u32) {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct SystemTime {
            year: u16, month: u16, _dow: u16, day: u16,
            hour: u16, minute: u16, second: u16, millis: u16,
        }
        unsafe extern "system" {
            fn GetLocalTime(st: *mut SystemTime);
        }
        let mut st = SystemTime { year: 0, month: 0, _dow: 0, day: 0, hour: 0, minute: 0, second: 0, millis: 0 };
        unsafe { GetLocalTime(&mut st); }
        (st.hour as u64, st.minute as u64, st.second as u64, st.millis as u32)
    }
    #[cfg(not(target_os = "windows"))]
    {
        #[repr(C)]
        struct Tm {
            tm_sec: i32, tm_min: i32, tm_hour: i32, tm_mday: i32,
            tm_mon: i32, tm_year: i32, tm_wday: i32, tm_yday: i32,
            tm_isdst: i32, tm_gmtoff: i64, tm_zone: *const i8,
        }
        unsafe extern "C" {
            fn time(t: *mut i64) -> i64;
            fn localtime_r(t: *const i64, result: *mut Tm) -> *mut Tm;
        }
        let mut t: i64 = 0;
        let mut tm = Tm { tm_sec: 0, tm_min: 0, tm_hour: 0, tm_mday: 0, tm_mon: 0, tm_year: 0, tm_wday: 0, tm_yday: 0, tm_isdst: 0, tm_gmtoff: 0, tm_zone: std::ptr::null() };
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
            year: u16, month: u16, _dow: u16, day: u16,
            hour: u16, minute: u16, second: u16, millis: u16,
        }
        unsafe extern "system" {
            fn GetLocalTime(st: *mut SystemTime);
        }
        let mut st = SystemTime { year: 0, month: 0, _dow: 0, day: 0, hour: 0, minute: 0, second: 0, millis: 0 };
        unsafe { GetLocalTime(&mut st); }
        (st.year, st.month, st.day)
    }
    #[cfg(not(target_os = "windows"))]
    {
        #[repr(C)]
        struct Tm {
            tm_sec: i32, tm_min: i32, tm_hour: i32, tm_mday: i32,
            tm_mon: i32, tm_year: i32, tm_wday: i32, tm_yday: i32,
            tm_isdst: i32, tm_gmtoff: i64, tm_zone: *const i8,
        }
        unsafe extern "C" {
            fn time(t: *mut i64) -> i64;
            fn localtime_r(t: *const i64, result: *mut Tm) -> *mut Tm;
        }
        let mut t: i64 = 0;
        let mut tm = Tm { tm_sec: 0, tm_min: 0, tm_hour: 0, tm_mday: 0, tm_mon: 0, tm_year: 0, tm_wday: 0, tm_yday: 0, tm_isdst: 0, tm_gmtoff: 0, tm_zone: std::ptr::null() };
        unsafe {
            time(&mut t);
            localtime_r(&t, &mut tm);
        }
        ((tm.tm_year + 1900) as u16, (tm.tm_mon + 1) as u16, tm.tm_mday as u16)
    }
}

fn builtin_timer(_args: &[Value]) -> Result<Value, RuntimeError> {
    let (h, m, s, ms) = local_time_parts();
    let secs_today = h * 3600 + m * 60 + s;
    Ok(Value::Single(secs_today as f64 + ms as f64 / 1000.0))
}

fn builtin_date(_args: &[Value]) -> Result<Value, RuntimeError> {
    let (year, month, day) = local_date_parts();
    Ok(Value::Str(format!("{:02}-{:02}-{:04}", month, day, year)))
}

fn builtin_time(_args: &[Value]) -> Result<Value, RuntimeError> {
    let (hours, mins, secs, _) = local_time_parts();
    Ok(Value::Str(format!("{:02}:{:02}:{:02}", hours, mins, secs)))
}

fn builtin_stub(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Integer(0))
}

fn builtin_environ(args: &[Value]) -> Result<Value, RuntimeError> {
    let name = args[0].to_string_val()?;
    let val = std::env::var(&name).unwrap_or_default();
    Ok(Value::Str(val))
}

// Binary conversion functions (MKI$/MKL$/MKS$/MKD$ and CVI/CVL/CVS/CVD)

/// Convert bytes to a string where each byte becomes a char (latin-1 style)
fn bytes_to_basic_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Convert a BASIC binary string back to bytes
fn basic_string_to_bytes(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u8).collect()
}

fn builtin_mki(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()? as i16;
    Ok(Value::Str(bytes_to_basic_string(&n.to_le_bytes())))
}

fn builtin_mkl(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()? as i32;
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

fn builtin_cvi(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let bytes = basic_string_to_bytes(&s);
    if bytes.len() < 2 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "CVI requires a 2-byte string".into(),
        });
    }
    let n = i16::from_le_bytes([bytes[0], bytes[1]]);
    Ok(Value::Integer(n as i64))
}

fn builtin_cvl(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let bytes = basic_string_to_bytes(&s);
    if bytes.len() < 4 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "CVL requires a 4-byte string".into(),
        });
    }
    let n = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    Ok(Value::Long(n as i64))
}

fn builtin_cvs(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let bytes = basic_string_to_bytes(&s);
    if bytes.len() < 4 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "CVS requires a 4-byte string".into(),
        });
    }
    let n = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    Ok(Value::Single(n as f64))
}

fn builtin_cvd(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let bytes = basic_string_to_bytes(&s);
    if bytes.len() < 8 {
        return Err(RuntimeError::IllegalFunctionCall {
            msg: "CVD requires an 8-byte string".into(),
        });
    }
    let n = f64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    Ok(Value::Double(n))
}
