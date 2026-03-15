/// FFI value operations for the Rice runtime.
///
/// RiceValue is passed as two i64 values: (tag, data).
/// Tag: 0=Integer, 1=Long, 2=Single, 3=Double, 4=String
/// Data: raw bits (i64 for ints, f64 bits for floats, pointer for strings)

use std::ffi::{CStr, CString};

use crate::ast::BinOp;
use crate::value::Value;

const TAG_INTEGER: i64 = 0;
const TAG_LONG: i64 = 1;
const TAG_SINGLE: i64 = 2;
const TAG_DOUBLE: i64 = 3;
const TAG_STRING: i64 = 4;

/// FFI-safe tagged value: two i64s returned in registers.
/// On x86_64: rax + rdx. On aarch64: x0 + x1.
#[repr(C)]
pub struct RiceValue {
    pub tag: i64,
    pub data: i64,
}

/// Convert a Value to RiceValue for FFI.
/// For strings, allocates a CString that must eventually be freed via rice_value_drop.
pub fn value_to_ffi(val: &Value) -> RiceValue {
    match val {
        Value::Integer(n) => RiceValue { tag: TAG_INTEGER, data: *n },
        Value::Long(n) => RiceValue { tag: TAG_LONG, data: *n },
        Value::Single(n) => RiceValue { tag: TAG_SINGLE, data: n.to_bits() as i64 },
        Value::Double(n) => RiceValue { tag: TAG_DOUBLE, data: n.to_bits() as i64 },
        Value::Str(s) => {
            let c_str = CString::new(s.as_str()).unwrap_or_default();
            let ptr = c_str.into_raw();
            RiceValue { tag: TAG_STRING, data: ptr as i64 }
        }
        Value::Record { .. } => RiceValue { tag: TAG_INTEGER, data: 0 },
    }
}

/// Convert RiceValue back to a Value.
/// For strings, reads (borrows) the C string without taking ownership.
pub fn ffi_to_value(tag: i64, data: i64) -> Value {
    match tag {
        TAG_INTEGER => Value::Integer(data),
        TAG_LONG => Value::Long(data),
        TAG_SINGLE => Value::Single(f64::from_bits(data as u64)),
        TAG_DOUBLE => Value::Double(f64::from_bits(data as u64)),
        TAG_STRING => {
            if data == 0 {
                Value::Str(String::new())
            } else {
                let c_str = unsafe { CStr::from_ptr(data as *const std::ffi::c_char) };
                Value::Str(c_str.to_string_lossy().into_owned())
            }
        }
        _ => Value::Integer(0),
    }
}

// --- extern "C" functions ---

#[unsafe(no_mangle)]
pub extern "C" fn rice_value_new_int(v: i64) -> RiceValue {
    RiceValue { tag: TAG_INTEGER, data: v }
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_value_new_double(v: f64) -> RiceValue {
    RiceValue { tag: TAG_DOUBLE, data: v.to_bits() as i64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_value_new_string(s: *const std::ffi::c_char) -> RiceValue {
    if s.is_null() {
        let c_str = CString::new("").unwrap();
        let ptr = c_str.into_raw();
        return RiceValue { tag: TAG_STRING, data: ptr as i64 };
    }
    // Copy the C string directly into a new CString (avoids intermediate Rust String)
    let c_str = unsafe { CStr::from_ptr(s) };
    let owned = c_str.to_owned();
    let ptr = owned.into_raw();
    RiceValue { tag: TAG_STRING, data: ptr as i64 }
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_value_drop(tag: i64, data: i64) {
    if tag == TAG_STRING && data != 0 {
        unsafe {
            let _ = CString::from_raw(data as *mut std::ffi::c_char);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_value_is_truthy(tag: i64, data: i64) -> i32 {
    let val = ffi_to_value(tag, data);
    match val.is_truthy() {
        Ok(true) => 1,
        _ => 0,
    }
}

fn i32_to_binop(op: i32) -> Option<BinOp> {
    match op {
        0 => Some(BinOp::Add),
        1 => Some(BinOp::Sub),
        2 => Some(BinOp::Mul),
        3 => Some(BinOp::Div),
        4 => Some(BinOp::IntDiv),
        5 => Some(BinOp::Mod),
        6 => Some(BinOp::Pow),
        7 => Some(BinOp::Eq),
        8 => Some(BinOp::Ne),
        9 => Some(BinOp::Lt),
        10 => Some(BinOp::Gt),
        11 => Some(BinOp::Le),
        12 => Some(BinOp::Ge),
        13 => Some(BinOp::And),
        14 => Some(BinOp::Or),
        15 => Some(BinOp::Xor),
        16 => Some(BinOp::Eqv),
        17 => Some(BinOp::Imp),
        _ => None,
    }
}

/// Perform a binary operation on two RiceValues.
#[unsafe(no_mangle)]
pub extern "C" fn rice_value_binop(
    ltag: i64,
    ldata: i64,
    op: i32,
    rtag: i64,
    rdata: i64,
) -> RiceValue {
    let lval = ffi_to_value(ltag, ldata);
    let rval = ffi_to_value(rtag, rdata);

    let binop = match i32_to_binop(op) {
        Some(b) => b,
        None => {
            eprintln!("rice runtime: invalid binop code {op}");
            return value_to_ffi(&Value::Integer(0));
        }
    };

    let result = eval_binop(&lval, binop, &rval);
    value_to_ffi(&result)
}

/// Perform a unary operation on a RiceValue.
#[unsafe(no_mangle)]
pub extern "C" fn rice_value_unary_op(
    tag: i64,
    data: i64,
    op: i32,
) -> RiceValue {
    let val = ffi_to_value(tag, data);
    let result = match op {
        0 => {
            // Neg
            match &val {
                Value::Integer(n) => Value::Integer(-n),
                Value::Long(n) => Value::Long(-n),
                Value::Single(n) => Value::Single(-n),
                Value::Double(n) => Value::Double(-n),
                _ => Value::Integer(0),
            }
        }
        1 => {
            // Not (bitwise)
            match &val {
                Value::Integer(n) => Value::Integer(!n),
                Value::Long(n) => Value::Long(!n),
                _ => {
                    let n = val.to_i64().unwrap_or(0);
                    Value::Integer(!n)
                }
            }
        }
        2 => val, // Pos (identity)
        _ => {
            eprintln!("rice runtime: invalid unary op code {op}");
            Value::Integer(0)
        }
    };
    value_to_ffi(&result)
}

/// Evaluate a binary operation, replicating interpreter semantics.
fn eval_binop(left: &Value, op: BinOp, right: &Value) -> Value {
    // String concatenation
    if let BinOp::Add = op {
        if let (Value::Str(a), Value::Str(b)) = (left, right) {
            return Value::Str(format!("{a}{b}"));
        }
    }

    // String comparison
    if let (Value::Str(a), Value::Str(b)) = (left, right) {
        return match op {
            BinOp::Eq => Value::Integer(if a == b { -1 } else { 0 }),
            BinOp::Ne => Value::Integer(if a != b { -1 } else { 0 }),
            BinOp::Lt => Value::Integer(if a < b { -1 } else { 0 }),
            BinOp::Gt => Value::Integer(if a > b { -1 } else { 0 }),
            BinOp::Le => Value::Integer(if a <= b { -1 } else { 0 }),
            BinOp::Ge => Value::Integer(if a >= b { -1 } else { 0 }),
            _ => {
                eprintln!("rice runtime: invalid string operation");
                Value::Integer(0)
            }
        };
    }

    // Numeric operations — determine result type first to avoid unnecessary f64 conversion
    let use_int = matches!(
        (left, right),
        (Value::Integer(_), Value::Integer(_))
            | (Value::Integer(_), Value::Long(_))
            | (Value::Long(_), Value::Integer(_))
            | (Value::Long(_), Value::Long(_))
    );

    if use_int {
        let li = left.to_i64().unwrap_or(0);
        let ri = right.to_i64().unwrap_or(0);
        return match op {
            BinOp::Add => Value::Integer(li + ri),
            BinOp::Sub => Value::Integer(li - ri),
            BinOp::Mul => Value::Integer(li * ri),
            BinOp::Div => {
                if ri == 0 {
                    eprintln!("rice runtime: division by zero");
                    Value::Integer(0)
                } else {
                    Value::Double(li as f64 / ri as f64)
                }
            }
            BinOp::IntDiv => {
                if ri == 0 {
                    eprintln!("rice runtime: division by zero");
                    Value::Integer(0)
                } else {
                    Value::Integer(li / ri)
                }
            }
            BinOp::Mod => {
                if ri == 0 {
                    eprintln!("rice runtime: division by zero");
                    Value::Integer(0)
                } else {
                    Value::Integer(li % ri)
                }
            }
            BinOp::Pow => Value::Double((li as f64).powf(ri as f64)),
            BinOp::Eq => Value::Integer(if li == ri { -1 } else { 0 }),
            BinOp::Ne => Value::Integer(if li != ri { -1 } else { 0 }),
            BinOp::Lt => Value::Integer(if li < ri { -1 } else { 0 }),
            BinOp::Gt => Value::Integer(if li > ri { -1 } else { 0 }),
            BinOp::Le => Value::Integer(if li <= ri { -1 } else { 0 }),
            BinOp::Ge => Value::Integer(if li >= ri { -1 } else { 0 }),
            BinOp::And => Value::Integer(li & ri),
            BinOp::Or => Value::Integer(li | ri),
            BinOp::Xor => Value::Integer(li ^ ri),
            BinOp::Eqv => Value::Integer(!(li ^ ri)),
            BinOp::Imp => Value::Integer(!li | ri),
        };
    }

    // Float path
    let lf = match left.to_f64() {
        Ok(v) => v,
        Err(_) => return Value::Integer(0),
    };
    let rf = match right.to_f64() {
        Ok(v) => v,
        Err(_) => return Value::Integer(0),
    };

    match op {
        BinOp::Add => Value::Double(lf + rf),
        BinOp::Sub => Value::Double(lf - rf),
        BinOp::Mul => Value::Double(lf * rf),
        BinOp::Div => {
            if rf == 0.0 {
                eprintln!("rice runtime: division by zero");
                Value::Integer(0)
            } else {
                Value::Double(lf / rf)
            }
        }
        BinOp::IntDiv => {
            if rf == 0.0 {
                eprintln!("rice runtime: division by zero");
                Value::Integer(0)
            } else {
                Value::Integer((lf as i64) / (rf as i64))
            }
        }
        BinOp::Mod => {
            if rf == 0.0 {
                eprintln!("rice runtime: division by zero");
                Value::Integer(0)
            } else {
                Value::Integer((lf as i64) % (rf as i64))
            }
        }
        BinOp::Pow => Value::Double(lf.powf(rf)),
        BinOp::Eq => Value::Integer(if lf == rf { -1 } else { 0 }),
        BinOp::Ne => Value::Integer(if lf != rf { -1 } else { 0 }),
        BinOp::Lt => Value::Integer(if lf < rf { -1 } else { 0 }),
        BinOp::Gt => Value::Integer(if lf > rf { -1 } else { 0 }),
        BinOp::Le => Value::Integer(if lf <= rf { -1 } else { 0 }),
        BinOp::Ge => Value::Integer(if lf >= rf { -1 } else { 0 }),
        BinOp::And => Value::Integer((lf as i64) & (rf as i64)),
        BinOp::Or => Value::Integer((lf as i64) | (rf as i64)),
        BinOp::Xor => Value::Integer((lf as i64) ^ (rf as i64)),
        BinOp::Eqv => Value::Integer(!((lf as i64) ^ (rf as i64))),
        BinOp::Imp => Value::Integer(!(lf as i64) | (rf as i64)),
    }
}
