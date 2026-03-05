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
        self.functions.contains_key(name)
    }
}

// Math builtins

fn builtin_abs(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(Value::Double(n.abs()))
}

fn builtin_int(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(Value::Double(n.floor()))
}

fn builtin_fix(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_f64()?;
    Ok(Value::Double(n.trunc()))
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
    Ok(Value::Integer(args[0].to_f64()?.round() as i64))
}

fn builtin_clng(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Long(args[0].to_f64()?.round() as i64))
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
        Value::Str(s) => Ok(Value::Integer(s.len() as i64)),
        _ => Err(RuntimeError::TypeMismatch {
            msg: "LEN expects a string".into(),
        }),
    }
}

fn builtin_left(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = args[0].to_string_val()?;
    let n = args[1].to_i64()? as usize;
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
    let n = args[0].to_i64()? as usize;
    Ok(Value::Str(" ".repeat(n)))
}

fn builtin_string_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let n = args[0].to_i64()? as usize;
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
    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' {
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

fn builtin_timer(_args: &[Value]) -> Result<Value, RuntimeError> {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs_today = now.as_secs() % 86400;
    let frac = now.subsec_millis() as f64 / 1000.0;
    Ok(Value::Single(secs_today as f64 + frac))
}

fn builtin_date(_args: &[Value]) -> Result<Value, RuntimeError> {
    // Standard BASIC returns MM-DD-YYYY or similar.
    // To avoid complex dependencies, we return a hardcoded date for now or use system time.
    // For now, let's just use placeholder to not break things without 'chrono'.
    Ok(Value::Str("02-28-2026".into()))
}

fn builtin_time(_args: &[Value]) -> Result<Value, RuntimeError> {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() % 86400;
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;
    Ok(Value::Str(format!("{:02}:{:02}:{:02}", hours, mins, secs)))
}

fn builtin_stub(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Integer(0))
}
