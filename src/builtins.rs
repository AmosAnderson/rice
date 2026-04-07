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
        reg.register("ROUND", builtin_round, 1);
        // ANSI math additions
        reg.register("ASIN", builtin_asin, 1);
        reg.register("ACOS", builtin_acos, 1);
        reg.register("COT", builtin_cot, 1);
        reg.register("CSC", builtin_csc, 1);
        reg.register("SEC", builtin_sec, 1);
        reg.register("ANGLE", builtin_angle, 2);
        reg.register("CEIL", builtin_ceil, 1);
        reg.register("TRUNCATE", builtin_truncate, 1);
        reg.register("REMAINDER", builtin_remainder, 2);
        reg.register("MAXNUM", builtin_maxnum, 0);
        reg.register("PI", builtin_pi, 0);
        // RND is handled as a stateful function in the interpreter

        // String
        reg.register("LEN", builtin_len, 1);
        reg.register("INSTR", builtin_instr, 0); // 2 or 3 args
        reg.register("LTRIM$", builtin_ltrim, 1);
        reg.register("RTRIM$", builtin_rtrim, 1);
        reg.register("SPACE$", builtin_space, 1);
        reg.register("STRING$", builtin_string_fn, 2);
        reg.register("CHR$", builtin_chr, 1);
        reg.register("ASC", builtin_asc, 1);
        reg.register("STR$", builtin_str, 1);
        reg.register("VAL", builtin_val, 1);

        // Misc
        reg.register("LBOUND", builtin_stub, 0);
        reg.register("UBOUND", builtin_stub, 0);
        reg.register("TIMER", builtin_timer, 0);
        reg.register("DATE$", builtin_date, 0);
        reg.register("TIME$", builtin_time, 0);
        reg.register("COMMAND$", builtin_stub, 0);
        reg.register("ENVIRON$", builtin_environ, 1);

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
    Ok(Value::Numeric(args[0].to_f64()?.round()))
}

fn builtin_asin(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    if !(-1.0..=1.0).contains(&n) {
        return Err(RuntimeError::IllegalFunctionCall { msg: "ASIN argument out of range".into() });
    }
    Ok(Value::Numeric(n.asin()))
}

fn builtin_acos(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    if !(-1.0..=1.0).contains(&n) {
        return Err(RuntimeError::IllegalFunctionCall { msg: "ACOS argument out of range".into() });
    }
    Ok(Value::Numeric(n.acos()))
}

fn builtin_cot(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    let t = n.tan();
    if t == 0.0 { return Err(RuntimeError::DivisionByZero); }
    Ok(Value::Numeric(1.0 / t))
}

fn builtin_csc(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    let s = n.sin();
    if s == 0.0 { return Err(RuntimeError::DivisionByZero); }
    Ok(Value::Numeric(1.0 / s))
}

fn builtin_sec(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    let c = n.cos();
    if c == 0.0 { return Err(RuntimeError::DivisionByZero); }
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
    Ok(Value::Numeric(args[0].to_f64()?.trunc()))
}

fn builtin_remainder(args: &[Value]) -> Result<Value, RuntimeError> {
    let a = args[0].to_f64()?;
    let b = args[1].to_f64()?;
    if b == 0.0 { return Err(RuntimeError::DivisionByZero); }
    Ok(Value::Numeric(a % b))
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
            let pos = haystack.find(&needle).map(|p| p + 1).unwrap_or(0);
            Ok(Value::Numeric(pos as f64))
        }
        3 => {
            let start = (args[0].to_i64()? - 1).max(0) as usize;
            let haystack = args[1].to_string_val()?;
            let needle = args[2].to_string_val()?;
            let pos = haystack[start.min(haystack.len())..]
                .find(&needle)
                .map(|p| p + start + 1)
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
    Ok(Value::Numeric(s.as_bytes()[0] as f64))
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
    // Try integer first, then float
    if let Ok(n) = s.parse::<i64>() {
        return Ok(Value::Numeric(n as f64));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(Value::Numeric(n));
    }
    // BASIC returns 0 for non-numeric strings after parsing leading digits
    let mut num_str = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_digit() || ch == '.' || ((ch == '-' || ch == '+') && i == 0) {
            num_str.push(ch);
        } else {
            break;
        }
    }
    if num_str.is_empty() {
        Ok(Value::Numeric(0.0))
    } else {
        Ok(Value::Numeric(num_str.parse::<f64>().unwrap_or(0.0)))
    }
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
    Ok(Value::Numeric(secs_today as f64 + ms as f64 / 1000.0))
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
    Ok(Value::Numeric(0.0))
}

fn builtin_environ(args: &[Value]) -> Result<Value, RuntimeError> {
    let name = args[0].to_string_val()?;
    let val = std::env::var(&name).unwrap_or_default();
    Ok(Value::Str(val))
}

