/// FFI I/O operations for the Rice runtime.
///
/// Handles PRINT, INPUT, console, file I/O, arrays, DATA, and all other
/// runtime state for compiled programs.

use std::collections::HashMap;
use std::ffi::CStr;
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};

use crate::runtime::value_ffi::{ffi_to_value, value_to_ffi};
use crate::value::Value;

/// File mode for OPEN statement
#[derive(Debug, Clone, Copy, PartialEq)]
enum RtFileMode {
    Input,
    Output,
    Append,
    Random,
    Binary,
}

/// Runtime file handle
struct RtFileHandle {
    mode: RtFileMode,
    reader: Option<BufReader<std::fs::File>>,
    writer: Option<BufWriter<std::fs::File>>,
    rec_len: usize,
    eof_flag: bool,
    field_mappings: Vec<(usize, String)>, // (width, var_name)
}

/// Runtime state for compiled programs
pub struct RiceRuntime {
    // Print state
    print_col: usize,
    print_row: usize,
    output: Box<dyn Write>,
    screen_width: usize,
    screen_height: usize,
    current_fg: Option<u8>,
    current_bg: Option<u8>,

    // GOSUB
    gosub_stack: Vec<i64>,

    // DATA/READ
    data_values: Vec<Value>,
    data_pos: usize,
    data_label_positions: HashMap<String, usize>,

    // RNG
    rng_state: u64,
    last_rnd: f64,

    // Arrays (flattened key → Value)
    arrays: HashMap<String, Value>,
    option_base: i32,

    // File I/O
    file_handles: HashMap<i64, RtFileHandle>,

    // Field variable values (for GET/PUT with FIELD)
    field_vars: HashMap<String, Value>,

    // INPUT state (multi-variable input)
    input_values: Vec<Value>,

    // SHARED/STATIC
    global_vars: HashMap<String, Value>,
    static_vars: HashMap<String, HashMap<String, Value>>,

    // DEFtype
    deftype_map: [u8; 26], // 0=Single(default), 1=Integer, 2=Long, 3=Single, 4=Double, 5=String

    // TYPE instances (var_path -> field_name -> value)
    type_instances: HashMap<String, HashMap<String, Value>>,

    // Error handling
    error_code: i64,
    error_line: i64,

    // COMMON variables for CHAIN
    common_vars: Vec<(String, Value)>,
}

impl RiceRuntime {
    fn new() -> Self {
        Self {
            print_col: 0,
            print_row: 1,
            output: Box::new(std::io::stdout()),
            screen_width: 80,
            screen_height: 25,
            current_fg: None,
            current_bg: None,
            gosub_stack: Vec::new(),
            data_values: Vec::new(),
            data_pos: 0,
            data_label_positions: HashMap::new(),
            rng_state: 0x12345678_9ABCDEF0,
            last_rnd: 0.0,
            arrays: HashMap::new(),
            option_base: 0,
            file_handles: HashMap::new(),
            field_vars: HashMap::new(),
            input_values: Vec::new(),
            global_vars: HashMap::new(),
            static_vars: HashMap::new(),
            deftype_map: [0; 26],
            type_instances: HashMap::new(),
            error_code: 0,
            error_line: 0,
            common_vars: Vec::new(),
        }
    }

    fn print_value(&mut self, val: &Value) {
        let s = val.format_for_print();
        let _ = write!(self.output, "{s}");
        self.print_col += s.len();
    }

    fn print_comma(&mut self) {
        let next_zone = ((self.print_col / 14) + 1) * 14;
        let spaces = next_zone - self.print_col;
        let pad = " ".repeat(spaces);
        let _ = write!(self.output, "{pad}");
        self.print_col = next_zone;
    }

    fn print_newline(&mut self) {
        let _ = writeln!(self.output);
        self.print_col = 0;
        self.print_row += 1;
    }

    /// Dispatch a runtime call by name, returning (tag, data)
    fn dispatch(&mut self, name: &str, args: &[(i64, i64)]) -> (i64, i64) {
        let args_vals: Vec<Value> = args.iter().map(|&(t, d)| ffi_to_value(t, d)).collect();

        let result = match name {
            // --- GOSUB ---
            "rice_gosub_push" => {
                if let Some(&(_, data)) = args.first() {
                    self.gosub_stack.push(data);
                }
                Value::Integer(0)
            }
            "rice_gosub_pop" => {
                let id = self.gosub_stack.pop().unwrap_or(-1);
                Value::Integer(id)
            }

            // --- DATA/READ ---
            "rice_data_add" => {
                if let Some(val) = args_vals.into_iter().next() {
                    self.data_values.push(val);
                }
                Value::Integer(0)
            }
            "rice_data_read" => {
                if self.data_pos < self.data_values.len() {
                    let val = self.data_values[self.data_pos].clone();
                    self.data_pos += 1;
                    val
                } else {
                    eprintln!("rice runtime: READ past end of DATA");
                    Value::Integer(0)
                }
            }
            "rice_data_restore" => {
                if let Some(val) = args_vals.first() {
                    let label = val.to_string_val().unwrap_or_default();
                    if label.is_empty() {
                        self.data_pos = 0;
                    } else if let Some(&pos) = self.data_label_positions.get(&label) {
                        self.data_pos = pos;
                    } else {
                        self.data_pos = 0;
                    }
                } else {
                    self.data_pos = 0;
                }
                Value::Integer(0)
            }
            "rice_data_set_label_pos" => {
                // Called during init: associates a label with a data position
                if args_vals.len() >= 2 {
                    let label = args_vals[0].to_string_val().unwrap_or_default();
                    let pos = args_vals[1].to_i64().unwrap_or(0) as usize;
                    self.data_label_positions.insert(label, pos);
                }
                Value::Integer(0)
            }

            // --- RANDOMIZE/RND ---
            "rice_randomize" => {
                if let Some(val) = args_vals.first() {
                    if let Value::Str(s) = val {
                        if s == "__TIME_SEED__" {
                            // Use system time
                            let nanos = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .subsec_nanos();
                            self.rng_state = nanos as u64;
                        } else {
                            self.rng_state = 0; // string seed
                        }
                    } else {
                        let seed = val.to_f64().unwrap_or(0.0);
                        self.rng_state = seed.to_bits();
                    }
                }
                Value::Integer(0)
            }
            "rice_fn_rnd" => {
                let arg = args_vals.first().and_then(|v| v.to_f64().ok()).unwrap_or(1.0);
                if arg < 0.0 {
                    self.rng_state = arg.to_bits();
                    self.rng_step();
                    Value::Single(self.last_rnd)
                } else if arg == 0.0 {
                    Value::Single(self.last_rnd)
                } else {
                    self.rng_step();
                    Value::Single(self.last_rnd)
                }
            }

            // --- Arrays ---
            "rice_array_dim" => {
                // args: name, (upper, lower)* [, type_name]
                // For now just register the array name
                Value::Integer(0)
            }
            "rice_array_get" => {
                // args: name, indices...
                if let Some(name_val) = args_vals.first() {
                    let name = name_val.to_string_val().unwrap_or_default();
                    let indices: Vec<i64> = args_vals[1..].iter()
                        .filter_map(|v| v.to_i64().ok())
                        .collect();
                    let key = self.make_array_key(&name, &indices);
                    if let Some(val) = self.arrays.get(&key) {
                        val.clone()
                    } else {
                        // Auto-initialize
                        let default = self.default_for_array(&name);
                        self.arrays.insert(key, default.clone());
                        default
                    }
                } else {
                    Value::Integer(0)
                }
            }
            "rice_array_set" => {
                // args: name, indices..., value (last arg is the value)
                if args_vals.len() >= 2 {
                    let name = args_vals[0].to_string_val().unwrap_or_default();
                    let value = args_vals.last().unwrap().clone();
                    let indices: Vec<i64> = args_vals[1..args_vals.len()-1].iter()
                        .filter_map(|v| v.to_i64().ok())
                        .collect();
                    let key = self.make_array_key(&name, &indices);
                    self.arrays.insert(key, value);
                }
                Value::Integer(0)
            }
            "rice_array_redim" => {
                if let Some(name_val) = args_vals.first() {
                    let name = name_val.to_string_val().unwrap_or_default();
                    let prefix = format!("{}_", name);
                    let keys: Vec<String> = self.arrays.keys()
                        .filter(|k| k.starts_with(&prefix))
                        .cloned()
                        .collect();
                    for k in keys {
                        self.arrays.remove(&k);
                    }
                }
                Value::Integer(0)
            }
            "rice_array_erase" => {
                if let Some(name_val) = args_vals.first() {
                    let name = name_val.to_string_val().unwrap_or_default();
                    let prefix = format!("{}_", name);
                    let keys: Vec<String> = self.arrays.keys()
                        .filter(|k| k.starts_with(&prefix) || *k == &name)
                        .cloned()
                        .collect();
                    for k in keys {
                        self.arrays.remove(&k);
                    }
                }
                Value::Integer(0)
            }
            "rice_option_base" => {
                if let Some(val) = args_vals.first() {
                    self.option_base = val.to_i64().unwrap_or(0) as i32;
                }
                Value::Integer(0)
            }

            // --- INPUT ---
            "rice_input_start" => {
                // args: prompt, count, suffixes
                let prompt = args_vals.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
                let count = args_vals.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(0) as usize;
                let suffixes = args_vals.get(2).map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();

                if !prompt.is_empty() {
                    let _ = write!(self.output, "{}", prompt);
                }
                let _ = write!(self.output, "? ");
                let _ = self.output.flush();

                self.input_values.clear();
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                let line = line.trim_end_matches(|c| c == '\n' || c == '\r');

                let suffix_chars: Vec<char> = suffixes.chars().collect();
                let fields: Vec<&str> = if count <= 1 && suffix_chars.first() == Some(&'$') {
                    vec![line]
                } else {
                    line.split(',').collect()
                };

                for i in 0..count {
                    let field = fields.get(i).map(|s| s.trim()).unwrap_or("");
                    let suffix = suffix_chars.get(i).copied().unwrap_or(' ');
                    if suffix == '$' {
                        self.input_values.push(Value::Str(field.to_string()));
                    } else if let Ok(n) = field.parse::<i64>() {
                        self.input_values.push(Value::Integer(n));
                    } else if let Ok(n) = field.parse::<f64>() {
                        self.input_values.push(Value::Double(n));
                    } else {
                        self.input_values.push(Value::Str(field.to_string()));
                    }
                }
                self.print_col = 0;
                Value::Integer(0)
            }
            "rice_input_get" => {
                let idx = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0) as usize;
                self.input_values.get(idx).cloned().unwrap_or(Value::Str(String::new()))
            }

            // --- LINE INPUT ---
            "rice_line_input" => {
                let prompt = args_vals.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
                if !prompt.is_empty() {
                    let _ = write!(self.output, "{}", prompt);
                    let _ = self.output.flush();
                }
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                let line = line.trim_end_matches(|c| c == '\n' || c == '\r');
                self.print_col = 0;
                Value::Str(line.to_string())
            }

            // --- PRINT helpers ---
            "rice_print_tab" => {
                if let Some(val) = args_vals.first() {
                    let col = val.to_i64().unwrap_or(1) as usize;
                    let target = if col > 0 { col - 1 } else { 0 };
                    if target > self.print_col {
                        let spaces = target - self.print_col;
                        let _ = write!(self.output, "{}", " ".repeat(spaces));
                        self.print_col = target;
                    }
                }
                Value::Integer(0)
            }
            "rice_print_spc" => {
                if let Some(val) = args_vals.first() {
                    let n = val.to_i64().unwrap_or(0) as usize;
                    let _ = write!(self.output, "{}", " ".repeat(n));
                    self.print_col += n;
                }
                Value::Integer(0)
            }
            "rice_print_using" => {
                // args: format_string, items..., trailing_sep
                if args_vals.len() >= 2 {
                    let fmt = args_vals[0].to_string_val().unwrap_or_default();
                    let trailing = args_vals.last().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                    let items = &args_vals[1..args_vals.len()-1];
                    let formatted = crate::format_using::format_using(&fmt, items).unwrap_or_default();
                    let _ = write!(self.output, "{}", formatted);
                    self.print_col += formatted.len();
                    match trailing {
                        0 => self.print_newline(),
                        1 => {} // semicolon
                        2 => self.print_comma(),
                        _ => self.print_newline(),
                    }
                }
                Value::Integer(0)
            }

            // --- WRITE (console) ---
            "rice_write_values" => {
                for (i, val) in args_vals.iter().enumerate() {
                    if i > 0 {
                        let _ = write!(self.output, ",");
                    }
                    let _ = write!(self.output, "{}", Self::format_write_item(val));
                }
                let _ = writeln!(self.output);
                self.print_col = 0;
                Value::Integer(0)
            }

            // --- Sleep/Clear/Shell/FileSystem ---
            "rice_sleep" => {
                let secs = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                if secs > 0 {
                    std::thread::sleep(std::time::Duration::from_secs(secs as u64));
                }
                Value::Integer(0)
            }
            "rice_clear" => {
                // Clear all arrays and variables (runtime-managed ones)
                self.arrays.clear();
                self.data_pos = 0;
                Value::Integer(0)
            }
            "rice_shell" => {
                let cmd = args_vals.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
                if !cmd.is_empty() {
                    #[cfg(windows)]
                    let _ = std::process::Command::new("cmd").args(["/c", &cmd]).status();
                    #[cfg(not(windows))]
                    let _ = std::process::Command::new("sh").args(["-c", &cmd]).status();
                }
                Value::Integer(0)
            }
            "rice_name" => {
                if args_vals.len() >= 2 {
                    let old = args_vals[0].to_string_val().unwrap_or_default();
                    let new = args_vals[1].to_string_val().unwrap_or_default();
                    if let Err(e) = std::fs::rename(&old, &new) {
                        eprintln!("rice runtime: NAME error: {}", e);
                    }
                }
                Value::Integer(0)
            }
            "rice_kill" => {
                let path = args_vals.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
                if let Err(e) = std::fs::remove_file(&path) {
                    eprintln!("rice runtime: KILL error: {}", e);
                }
                Value::Integer(0)
            }
            "rice_mkdir" => {
                let path = args_vals.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
                if let Err(e) = std::fs::create_dir(&path) {
                    eprintln!("rice runtime: MKDIR error: {}", e);
                }
                Value::Integer(0)
            }
            "rice_rmdir" => {
                let path = args_vals.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
                if let Err(e) = std::fs::remove_dir(&path) {
                    eprintln!("rice runtime: RMDIR error: {}", e);
                }
                Value::Integer(0)
            }
            "rice_chdir" => {
                let path = args_vals.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
                if let Err(e) = std::env::set_current_dir(&path) {
                    eprintln!("rice runtime: CHDIR error: {}", e);
                }
                Value::Integer(0)
            }

            // --- Console operations ---
            "rice_cls" => {
                let _ = write!(self.output, "\x1b[2J\x1b[H");
                let _ = self.output.flush();
                self.print_row = 1;
                self.print_col = 0;
                Value::Integer(0)
            }
            "rice_beep" => {
                let _ = write!(self.output, "\x07");
                let _ = self.output.flush();
                Value::Integer(0)
            }
            "rice_locate" => {
                let row = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                let col = args_vals.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(0);
                if row > 0 && col > 0 {
                    let _ = write!(self.output, "\x1b[{};{}H", row, col);
                    self.print_row = row as usize;
                    self.print_col = (col - 1) as usize;
                } else if row > 0 {
                    let _ = write!(self.output, "\x1b[{};1H", row);
                    self.print_row = row as usize;
                }
                let _ = self.output.flush();
                Value::Integer(0)
            }
            "rice_color" => {
                let fg = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(-1);
                let bg = args_vals.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(-1);
                let mut seq = String::from("\x1b[");
                let mut need_sep = false;
                if fg >= 0 && fg <= 15 {
                    self.current_fg = Some(fg as u8);
                    seq.push_str(&Self::qb_fg_to_ansi(fg as u8).to_string());
                    need_sep = true;
                }
                if bg >= 0 && bg <= 15 {
                    self.current_bg = Some(bg as u8);
                    if need_sep { seq.push(';'); }
                    seq.push_str(&Self::qb_bg_to_ansi(bg as u8).to_string());
                }
                seq.push('m');
                if fg >= 0 || bg >= 0 {
                    let _ = write!(self.output, "{}", seq);
                }
                Value::Integer(0)
            }
            "rice_width" => {
                let cols = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                let rows = args_vals.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(0);
                if cols > 0 { self.screen_width = cols as usize; }
                if rows > 0 { self.screen_height = rows as usize; }
                Value::Integer(0)
            }
            "rice_view_print" => {
                let top = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                let bottom = args_vals.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(0);
                if top > 0 && bottom > 0 {
                    let _ = write!(self.output, "\x1b[{};{}r", top, bottom);
                } else {
                    let _ = write!(self.output, "\x1b[r");
                }
                Value::Integer(0)
            }
            "rice_fn_csrlin" => Value::Integer(self.print_row as i64),
            "rice_fn_pos" => Value::Integer((self.print_col + 1) as i64),
            "rice_fn_inkey" | "rice_fn_inkey$" => Value::Str(String::new()),
            "rice_fn_input$" => {
                // INPUT$(n) - read n characters
                let n = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(1) as usize;
                let mut buf = vec![0u8; n];
                let _ = std::io::stdin().read_exact(&mut buf);
                Value::Str(String::from_utf8_lossy(&buf).to_string())
            }
            "rice_fn_screen" => Value::Integer(0), // stub

            // --- Error handling ---
            "rice_fn_err" => Value::Integer(self.error_code),
            "rice_fn_erl" => Value::Integer(self.error_line),
            "rice_set_error_handler" | "rice_resume" | "rice_resume_next" | "rice_resume_label" => {
                // Simplified: just clear error state
                self.error_code = 0;
                self.error_line = 0;
                Value::Integer(0)
            }

            // --- String operations ---
            "rice_mid_assign" => {
                // args: var_value, start, length, replacement
                if args_vals.len() >= 4 {
                    let current = args_vals[0].to_string_val().unwrap_or_default();
                    let start = args_vals[1].to_i64().unwrap_or(1) as usize;
                    let max_len = args_vals[2].to_i64().unwrap_or(-1);
                    let replacement = args_vals[3].to_string_val().unwrap_or_default();

                    let mut chars: Vec<char> = current.chars().collect();
                    let repl_chars: Vec<char> = replacement.chars().collect();
                    let start_idx = if start > 0 { start - 1 } else { 0 };
                    let available = if start_idx < chars.len() { chars.len() - start_idx } else { 0 };
                    let replace_len = if max_len >= 0 {
                        std::cmp::min(std::cmp::min(max_len as usize, available), repl_chars.len())
                    } else {
                        std::cmp::min(available, repl_chars.len())
                    };
                    for i in 0..replace_len {
                        if start_idx + i < chars.len() {
                            chars[start_idx + i] = repl_chars[i];
                        }
                    }
                    Value::Str(chars.into_iter().collect())
                } else {
                    args_vals.first().cloned().unwrap_or(Value::Str(String::new()))
                }
            }
            "rice_lset" => {
                if args_vals.len() >= 2 {
                    let target_len = args_vals[0].to_string_val().unwrap_or_default().len();
                    let src = args_vals[1].to_string_val().unwrap_or_default();
                    if src.len() >= target_len {
                        Value::Str(src[..target_len].to_string())
                    } else {
                        let mut result = src;
                        while result.len() < target_len {
                            result.push(' ');
                        }
                        Value::Str(result)
                    }
                } else {
                    args_vals.first().cloned().unwrap_or(Value::Str(String::new()))
                }
            }
            "rice_rset" => {
                if args_vals.len() >= 2 {
                    let target_len = args_vals[0].to_string_val().unwrap_or_default().len();
                    let src = args_vals[1].to_string_val().unwrap_or_default();
                    if src.len() >= target_len {
                        Value::Str(src[src.len()-target_len..].to_string())
                    } else {
                        let pad = target_len - src.len();
                        let mut result = " ".repeat(pad);
                        result.push_str(&src);
                        Value::Str(result)
                    }
                } else {
                    args_vals.first().cloned().unwrap_or(Value::Str(String::new()))
                }
            }

            // --- SHARED/STATIC ---
            "rice_shared_get" => {
                let name = args_vals.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
                self.global_vars.get(&name).cloned().unwrap_or(Value::Integer(0))
            }
            "rice_shared_set" => {
                if args_vals.len() >= 2 {
                    let name = args_vals[0].to_string_val().unwrap_or_default();
                    let val = args_vals[1].clone();
                    self.global_vars.insert(name, val);
                }
                Value::Integer(0)
            }
            "rice_static_load" => {
                if args_vals.len() >= 2 {
                    let func_name = args_vals[0].to_string_val().unwrap_or_default();
                    let var_name = args_vals[1].to_string_val().unwrap_or_default();
                    self.static_vars.get(&func_name)
                        .and_then(|m| m.get(&var_name))
                        .cloned()
                        .unwrap_or(Value::Integer(0))
                } else {
                    Value::Integer(0)
                }
            }
            "rice_static_save" => {
                if args_vals.len() >= 3 {
                    let func_name = args_vals[0].to_string_val().unwrap_or_default();
                    let var_name = args_vals[1].to_string_val().unwrap_or_default();
                    let val = args_vals[2].clone();
                    self.static_vars.entry(func_name).or_default().insert(var_name, val);
                }
                Value::Integer(0)
            }

            // --- DEFtype ---
            "rice_deftype" => {
                if args_vals.len() >= 3 {
                    let start = args_vals[0].to_i64().unwrap_or(0) as u8;
                    let end = args_vals[1].to_i64().unwrap_or(0) as u8;
                    let type_id = args_vals[2].to_i64().unwrap_or(0) as u8;
                    let start_idx = start.wrapping_sub(b'A');
                    let end_idx = end.wrapping_sub(b'A');
                    for i in start_idx..=end_idx {
                        if (i as usize) < 26 {
                            self.deftype_map[i as usize] = type_id;
                        }
                    }
                }
                Value::Integer(0)
            }

            // --- TYPE ---
            "rice_type_define" => {
                // Registration handled at compile time; no-op at runtime
                Value::Integer(0)
            }
            "rice_type_create" => {
                // args: var_name, type_name
                if args_vals.len() >= 2 {
                    let var_name = args_vals[0].to_string_val().unwrap_or_default();
                    let _type_name = args_vals[1].to_string_val().unwrap_or_default();
                    // Initialize with empty field map
                    self.type_instances.entry(var_name).or_default();
                }
                Value::Integer(0)
            }
            "rice_member_get" | "rice_member_get_dynamic" => {
                // args: obj_path, field_name
                if args_vals.len() >= 2 {
                    let obj = args_vals[0].to_string_val().unwrap_or_default();
                    let field = args_vals[1].to_string_val().unwrap_or_default();
                    if let Some(fields) = self.type_instances.get(&obj) {
                        fields.get(&field).cloned().unwrap_or(Value::Integer(0))
                    } else {
                        Value::Integer(0)
                    }
                } else if let Some(val) = args_vals.first() {
                    // Legacy single-path mode
                    let path = val.to_string_val().unwrap_or_default();
                    if let Some(dot_pos) = path.find('.') {
                        let var = &path[..dot_pos];
                        let field = &path[dot_pos+1..];
                        if let Some(fields) = self.type_instances.get(var) {
                            fields.get(field).cloned().unwrap_or(Value::Integer(0))
                        } else {
                            Value::Integer(0)
                        }
                    } else {
                        Value::Integer(0)
                    }
                } else {
                    Value::Integer(0)
                }
            }
            "rice_member_set" | "rice_member_set_dynamic" => {
                // args: full_path (obj.field), value
                if args_vals.len() >= 2 {
                    let path = args_vals[0].to_string_val().unwrap_or_default();
                    let val = args_vals[1].clone();
                    if let Some(dot_pos) = path.find('.') {
                        let var = path[..dot_pos].to_string();
                        let field = path[dot_pos+1..].to_string();
                        self.type_instances.entry(var).or_default().insert(field, val);
                    }
                }
                Value::Integer(0)
            }
            "rice_type_copy" => {
                // args: src_name, dst_name — copy all fields from src type instance to dst
                if args_vals.len() >= 2 {
                    let src = args_vals[0].to_string_val().unwrap_or_default();
                    let dst = args_vals[1].to_string_val().unwrap_or_default();
                    if let Some(fields) = self.type_instances.get(&src).cloned() {
                        self.type_instances.insert(dst, fields);
                    }
                }
                Value::Integer(0)
            }
            "rice_build_array_path" => {
                // args: name, indices...
                if let Some(name_val) = args_vals.first() {
                    let name = name_val.to_string_val().unwrap_or_default();
                    let indices: Vec<String> = args_vals[1..].iter()
                        .map(|v| v.to_i64().unwrap_or(0).to_string())
                        .collect();
                    Value::Str(format!("{}_{}", name, indices.join("_")))
                } else {
                    Value::Str(String::new())
                }
            }
            "rice_build_member_path" => {
                // args: obj_path, field_name
                if args_vals.len() >= 2 {
                    let obj = args_vals[0].to_string_val().unwrap_or_default();
                    let field = args_vals[1].to_string_val().unwrap_or_default();
                    Value::Str(format!("{}.{}", obj, field))
                } else {
                    Value::Str(String::new())
                }
            }

            // --- File I/O ---
            "rice_file_open" => {
                self.rt_file_open(&args_vals);
                Value::Integer(0)
            }
            "rice_file_close" => {
                let fnum = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                if let Some(mut fh) = self.file_handles.remove(&fnum) {
                    if let Some(ref mut w) = fh.writer {
                        let _ = w.flush();
                    }
                }
                Value::Integer(0)
            }
            "rice_file_close_all" => {
                let keys: Vec<i64> = self.file_handles.keys().cloned().collect();
                for k in keys {
                    if let Some(mut fh) = self.file_handles.remove(&k) {
                        if let Some(ref mut w) = fh.writer {
                            let _ = w.flush();
                        }
                    }
                }
                Value::Integer(0)
            }
            "rice_file_print" => {
                self.rt_file_print(&args_vals);
                Value::Integer(0)
            }
            "rice_file_print_using" => {
                self.rt_file_print_using(&args_vals);
                Value::Integer(0)
            }
            "rice_file_write" => {
                self.rt_file_write(&args_vals);
                Value::Integer(0)
            }
            "rice_file_input_var" => {
                self.rt_file_input_var(&args_vals)
            }
            "rice_file_line_input" => {
                self.rt_file_line_input(&args_vals)
            }
            "rice_file_get" => {
                self.rt_file_get(&args_vals)
            }
            "rice_file_get_fielded" => {
                self.rt_file_get_fielded(&args_vals);
                Value::Integer(0)
            }
            "rice_file_put" => {
                self.rt_file_put(&args_vals);
                Value::Integer(0)
            }
            "rice_file_put_fielded" => {
                self.rt_file_put_fielded(&args_vals);
                Value::Integer(0)
            }
            "rice_file_field" => {
                self.rt_file_field(&args_vals);
                Value::Integer(0)
            }
            "rice_file_seek" => {
                self.rt_file_seek(&args_vals);
                Value::Integer(0)
            }
            "rice_fn_eof" => {
                let fnum = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                if let Some(fh) = self.file_handles.get(&fnum) {
                    Value::Integer(if fh.eof_flag { -1 } else { 0 })
                } else {
                    Value::Integer(-1)
                }
            }
            "rice_fn_lof" => {
                let fnum = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                let mut lof_val = 0i64;
                if let Some(fh) = self.file_handles.get_mut(&fnum) {
                    if let Some(ref mut r) = fh.reader {
                        if let Ok(pos) = r.stream_position() {
                            if let Ok(end) = r.seek(SeekFrom::End(0)) {
                                let _ = r.seek(SeekFrom::Start(pos));
                                lof_val = end as i64;
                            }
                        }
                    }
                }
                Value::Long(lof_val)
            }
            "rice_fn_loc" => {
                let fnum = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                let mut loc_val = 0i64;
                if let Some(fh) = self.file_handles.get_mut(&fnum) {
                    if let Some(ref mut r) = fh.reader {
                        if let Ok(pos) = r.stream_position() {
                            loc_val = if fh.mode == RtFileMode::Random {
                                (pos as i64) / (fh.rec_len as i64)
                            } else {
                                pos as i64
                            };
                        }
                    }
                }
                Value::Long(loc_val)
            }
            "rice_fn_seek" => {
                let fnum = args_vals.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
                let mut seek_val = 1i64;
                if let Some(fh) = self.file_handles.get_mut(&fnum) {
                    if let Some(ref mut r) = fh.reader {
                        if let Ok(pos) = r.stream_position() {
                            seek_val = if fh.mode == RtFileMode::Random {
                                (pos as i64) / (fh.rec_len as i64) + 1
                            } else {
                                pos as i64 + 1
                            };
                        }
                    }
                }
                Value::Long(seek_val)
            }
            "rice_fn_freefile" => {
                let mut fnum = 1i64;
                while self.file_handles.contains_key(&fnum) {
                    fnum += 1;
                }
                Value::Integer(fnum)
            }

            // --- CHAIN/COMMON ---
            "rice_common_register" => {
                if args_vals.len() >= 2 {
                    let name = args_vals[0].to_string_val().unwrap_or_default();
                    let val = args_vals[1].clone();
                    self.common_vars.push((name, val));
                }
                Value::Integer(0)
            }
            "rice_chain" => {
                let _filespec = args_vals.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
                // CHAIN requires embedding the interpreter — simplified stub
                eprintln!("rice runtime: CHAIN not fully supported in compiled mode");
                Value::Integer(0)
            }

            _ => {
                eprintln!("rice runtime: unknown runtime call '{name}'");
                Value::Integer(0)
            }
        };

        value_to_ffi(&result)
    }

    // --- Helper methods ---

    /// Format a single value for WRITE output (quoted strings, raw numbers)
    fn format_write_item(val: &Value) -> String {
        match val {
            Value::Str(s) => format!("\"{}\"", s),
            Value::Integer(n) => n.to_string(),
            other => {
                let f = other.to_f64().unwrap_or(0.0);
                if f == f.floor() && f.abs() < 1e15 {
                    (f as i64).to_string()
                } else {
                    f.to_string()
                }
            }
        }
    }

    fn rng_step(&mut self) {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r = ((self.rng_state >> 33) as f64) / (2147483648.0);
        self.last_rnd = r as f32 as f64; // QBasic-compatible: narrow through f32
    }

    fn make_array_key(&self, name: &str, indices: &[i64]) -> String {
        let idx_part: String = indices.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("_");
        format!("{}_{}", name, idx_part)
    }

    fn default_for_array(&self, name: &str) -> Value {
        // Check suffix for type
        if name.ends_with('$') {
            Value::Str(String::new())
        } else if name.ends_with('%') {
            Value::Integer(0)
        } else if name.ends_with('&') {
            Value::Long(0)
        } else if name.ends_with('#') {
            Value::Double(0.0)
        } else if name.ends_with('!') {
            Value::Single(0.0)
        } else {
            Value::Integer(0)
        }
    }

    fn qb_fg_to_ansi(c: u8) -> u8 {
        match c {
            0 => 30, 1 => 34, 2 => 32, 3 => 36,
            4 => 31, 5 => 35, 6 => 33, 7 => 37,
            8 => 90, 9 => 94, 10 => 92, 11 => 96,
            12 => 91, 13 => 95, 14 => 93, 15 => 97,
            _ => 37,
        }
    }

    fn qb_bg_to_ansi(c: u8) -> u8 {
        match c {
            0 => 40, 1 => 44, 2 => 42, 3 => 46,
            4 => 41, 5 => 45, 6 => 43, 7 => 47,
            8 => 100, 9 => 104, 10 => 102, 11 => 106,
            12 => 101, 13 => 105, 14 => 103, 15 => 107,
            _ => 40,
        }
    }

    // --- File I/O helpers ---

    fn rt_file_open(&mut self, args: &[Value]) {
        let filename = args.first().map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
        let fnum = args.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(1);
        let mode_val = args.get(2).and_then(|v| v.to_i64().ok()).unwrap_or(0);
        let rec_len = args.get(3).and_then(|v| v.to_i64().ok()).unwrap_or(128) as usize;

        let mode = match mode_val {
            0 => RtFileMode::Input,
            1 => RtFileMode::Output,
            2 => RtFileMode::Append,
            3 => RtFileMode::Random,
            4 => RtFileMode::Binary,
            _ => RtFileMode::Input,
        };

        let (reader, writer) = match mode {
            RtFileMode::Input => {
                let f = std::fs::File::open(&filename).ok();
                (f.map(BufReader::new), None)
            }
            RtFileMode::Output => {
                let f = std::fs::File::create(&filename).ok();
                (None, f.map(BufWriter::new))
            }
            RtFileMode::Append => {
                let f = std::fs::OpenOptions::new().append(true).create(true).open(&filename).ok();
                (None, f.map(BufWriter::new))
            }
            RtFileMode::Random | RtFileMode::Binary => {
                let f = std::fs::OpenOptions::new()
                    .read(true).write(true).create(true)
                    .open(&filename).ok();
                if let Some(file) = f {
                    let f2 = file.try_clone().ok();
                    (Some(BufReader::new(file)), f2.map(BufWriter::new))
                } else {
                    (None, None)
                }
            }
        };

        self.file_handles.insert(fnum, RtFileHandle {
            mode,
            reader,
            writer,
            rec_len,
            eof_flag: false,
            field_mappings: Vec::new(),
        });
    }

    fn rt_file_print(&mut self, args: &[Value]) {
        if args.is_empty() { return; }
        let fnum = args[0].to_i64().unwrap_or(0);
        let trailing = args.last().and_then(|v| v.to_i64().ok()).unwrap_or(0);

        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            if let Some(ref mut writer) = fh.writer {
                // args[1..last-1] are pairs: (value, item_type)
                let items = &args[1..args.len()-1];
                let mut i = 0;
                while i + 1 < items.len() {
                    let val = &items[i];
                    let item_type = items[i+1].to_i64().unwrap_or(0);
                    match item_type {
                        0 => { // expression
                            let s = val.format_for_print();
                            let _ = write!(writer, "{}", s);
                        }
                        1 => { // comma
                            let _ = write!(writer, "\t");
                        }
                        2 => { // tab
                            let n = val.to_i64().unwrap_or(1);
                            let _ = write!(writer, "{}", " ".repeat(n as usize));
                        }
                        3 => { // spc
                            let n = val.to_i64().unwrap_or(0);
                            let _ = write!(writer, "{}", " ".repeat(n as usize));
                        }
                        _ => {}
                    }
                    i += 2;
                }
                match trailing {
                    0 => { let _ = writeln!(writer); }
                    1 => {} // semicolon
                    2 => { let _ = write!(writer, "\t"); }
                    _ => { let _ = writeln!(writer); }
                }
            }
        }
    }

    fn rt_file_print_using(&mut self, args: &[Value]) {
        if args.len() < 3 { return; }
        let fnum = args[0].to_i64().unwrap_or(0);
        let fmt = args[1].to_string_val().unwrap_or_default();
        let trailing = args.last().and_then(|v| v.to_i64().ok()).unwrap_or(0);
        let items = &args[2..args.len()-1];

        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            if let Some(ref mut writer) = fh.writer {
                let formatted = crate::format_using::format_using(&fmt, items).unwrap_or_default();
                let _ = write!(writer, "{}", formatted);
                match trailing {
                    0 => { let _ = writeln!(writer); }
                    1 => {}
                    2 => { let _ = write!(writer, "\t"); }
                    _ => { let _ = writeln!(writer); }
                }
            }
        }
    }

    fn rt_file_write(&mut self, args: &[Value]) {
        if args.is_empty() { return; }
        let fnum = args[0].to_i64().unwrap_or(0);
        let items = &args[1..];

        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            if let Some(ref mut writer) = fh.writer {
                for (i, val) in items.iter().enumerate() {
                    if i > 0 {
                        let _ = write!(writer, ",");
                    }
                    let _ = write!(writer, "{}", Self::format_write_item(val));
                }
                let _ = writeln!(writer);
            }
        }
    }

    fn rt_file_input_var(&mut self, args: &[Value]) -> Value {
        let fnum = args.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
        let suffix = args.get(1).map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
        let suffix_char = suffix.chars().next().unwrap_or(' ');

        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            if let Some(ref mut reader) = fh.reader {
                let field = Self::read_next_field(reader, &mut fh.eof_flag);
                // Check if we've reached EOF after this read
                if !fh.eof_flag {
                    if let Ok(buf) = reader.fill_buf() {
                        if buf.is_empty() {
                            fh.eof_flag = true;
                        }
                    }
                }
                if suffix_char == '$' {
                    Value::Str(field)
                } else if let Ok(n) = field.parse::<i64>() {
                    Value::Integer(n)
                } else if let Ok(n) = field.parse::<f64>() {
                    Value::Double(n)
                } else {
                    Value::Str(field)
                }
            } else {
                Value::Str(String::new())
            }
        } else {
            Value::Str(String::new())
        }
    }

    fn read_next_field(reader: &mut BufReader<std::fs::File>, eof_flag: &mut bool) -> String {
        let mut result = String::new();
        let mut in_quotes = false;
        let mut started = false;

        loop {
            let buf = match reader.fill_buf() {
                Ok(buf) if !buf.is_empty() => buf.to_vec(),
                _ => {
                    *eof_flag = true;
                    break;
                }
            };

            let mut consumed = 0;
            for &byte in &buf {
                consumed += 1;
                let ch = byte as char;

                if !started && ch == '"' {
                    in_quotes = true;
                    started = true;
                    continue;
                }
                started = true;

                if in_quotes {
                    if ch == '"' {
                        in_quotes = false;
                        continue;
                    }
                    result.push(ch);
                } else {
                    if ch == ',' || ch == '\n' {
                        reader.consume(consumed);
                        return result.trim_end_matches('\r').to_string();
                    }
                    result.push(ch);
                }
            }
            reader.consume(consumed);
        }
        result.trim_end_matches('\r').to_string()
    }

    fn rt_file_line_input(&mut self, args: &[Value]) -> Value {
        let fnum = args.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            if let Some(ref mut reader) = fh.reader {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        fh.eof_flag = true;
                        Value::Str(String::new())
                    }
                    Ok(_) => {
                        let trimmed = line.trim_end_matches(|c| c == '\n' || c == '\r');
                        // Check if we've reached EOF after this read
                        if let Ok(buf) = reader.fill_buf() {
                            if buf.is_empty() {
                                fh.eof_flag = true;
                            }
                        }
                        Value::Str(trimmed.to_string())
                    }
                    Err(_) => {
                        fh.eof_flag = true;
                        Value::Str(String::new())
                    }
                }
            } else {
                Value::Str(String::new())
            }
        } else {
            Value::Str(String::new())
        }
    }

    fn rt_file_get(&mut self, args: &[Value]) -> Value {
        let fnum = args.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
        let record = args.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(-1);
        let _var_name = args.get(2).map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();

        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            if record > 0 {
                let byte_pos = if fh.mode == RtFileMode::Random {
                    ((record - 1) * fh.rec_len as i64) as u64
                } else {
                    (record - 1) as u64
                };
                if let Some(ref mut r) = fh.reader {
                    let _ = r.seek(SeekFrom::Start(byte_pos));
                }
            }
            if let Some(ref mut reader) = fh.reader {
                let mut buf = vec![0u8; fh.rec_len];
                match reader.read(&mut buf) {
                    Ok(0) => {
                        fh.eof_flag = true;
                        Value::Str(String::new())
                    }
                    Ok(n) => {
                        buf.truncate(n);
                        Value::Str(String::from_utf8_lossy(&buf).to_string())
                    }
                    Err(_) => {
                        fh.eof_flag = true;
                        Value::Str(String::new())
                    }
                }
            } else {
                Value::Str(String::new())
            }
        } else {
            Value::Str(String::new())
        }
    }

    fn rt_file_get_fielded(&mut self, args: &[Value]) {
        let fnum = args.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
        let record = args.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(-1);

        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            if record > 0 {
                let byte_pos = ((record - 1) * fh.rec_len as i64) as u64;
                if let Some(ref mut r) = fh.reader {
                    let _ = r.seek(SeekFrom::Start(byte_pos));
                }
            }
            let rec_len = fh.rec_len;
            let mappings = fh.field_mappings.clone();
            if let Some(ref mut reader) = fh.reader {
                let mut buf = vec![0u8; rec_len];
                let bytes_read = reader.read(&mut buf).unwrap_or(0);
                if bytes_read == 0 {
                    fh.eof_flag = true;
                    return;
                }
                let mut offset = 0;
                for (width, var_name) in &mappings {
                    let end = std::cmp::min(offset + width, buf.len());
                    let field_data = &buf[offset..end];
                    let s = String::from_utf8_lossy(field_data).to_string();
                    self.field_vars.insert(var_name.clone(), Value::Str(s));
                    offset = end;
                }
            }
        }
    }

    fn rt_file_put(&mut self, args: &[Value]) {
        let fnum = args.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
        let record = args.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(-1);
        let _var_name = args.get(2).map(|v| v.to_string_val().unwrap_or_default()).unwrap_or_default();
        let val = args.get(3).cloned().unwrap_or(Value::Str(String::new()));

        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            if record > 0 {
                let byte_pos = if fh.mode == RtFileMode::Random {
                    ((record - 1) * fh.rec_len as i64) as u64
                } else {
                    (record - 1) as u64
                };
                if let Some(ref mut w) = fh.writer {
                    let _ = w.flush();
                    let _ = w.seek(SeekFrom::Start(byte_pos));
                }
            }
            if let Some(ref mut writer) = fh.writer {
                let s = val.to_string_val().unwrap_or_default();
                let mut bytes = s.into_bytes();
                if fh.mode == RtFileMode::Random {
                    bytes.resize(fh.rec_len, b' ');
                }
                let _ = writer.write_all(&bytes);
                let _ = writer.flush();
            }
        }
    }

    fn rt_file_put_fielded(&mut self, args: &[Value]) {
        let fnum = args.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
        let record = args.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(-1);

        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            if record > 0 {
                let byte_pos = ((record - 1) * fh.rec_len as i64) as u64;
                if let Some(ref mut w) = fh.writer {
                    let _ = w.flush();
                    let _ = w.seek(SeekFrom::Start(byte_pos));
                }
            }
            let rec_len = fh.rec_len;
            let mappings = fh.field_mappings.clone();
            let mut buf = vec![b' '; rec_len];
            let mut offset = 0;
            for (width, var_name) in &mappings {
                let val = self.field_vars.get(var_name)
                    .map(|v| v.to_string_val().unwrap_or_default())
                    .unwrap_or_else(|| " ".repeat(*width));
                let bytes = val.as_bytes();
                let copy_len = std::cmp::min(*width, bytes.len());
                buf[offset..offset+copy_len].copy_from_slice(&bytes[..copy_len]);
                offset += width;
            }
            if let Some(ref mut writer) = fh.writer {
                let _ = writer.write_all(&buf);
                let _ = writer.flush();
            }
        }
    }

    fn rt_file_field(&mut self, args: &[Value]) {
        if args.is_empty() { return; }
        let fnum = args[0].to_i64().unwrap_or(0);
        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            fh.field_mappings.clear();
            let pairs = &args[1..];
            let mut i = 0;
            while i + 1 < pairs.len() {
                let width = pairs[i].to_i64().unwrap_or(0) as usize;
                let var_name = pairs[i+1].to_string_val().unwrap_or_default();
                // Initialize field variable with spaces
                self.field_vars.insert(var_name.clone(), Value::Str(" ".repeat(width)));
                fh.field_mappings.push((width, var_name));
                i += 2;
            }
        }
    }

    fn rt_file_seek(&mut self, args: &[Value]) {
        let fnum = args.first().and_then(|v| v.to_i64().ok()).unwrap_or(0);
        let position = args.get(1).and_then(|v| v.to_i64().ok()).unwrap_or(1);

        if let Some(fh) = self.file_handles.get_mut(&fnum) {
            let byte_pos = if fh.mode == RtFileMode::Random {
                ((position - 1) * fh.rec_len as i64) as u64
            } else {
                (position - 1) as u64
            };
            if let Some(ref mut w) = fh.writer {
                let _ = w.flush();
                let _ = w.seek(SeekFrom::Start(byte_pos));
            }
            if let Some(ref mut r) = fh.reader {
                let _ = r.seek(SeekFrom::Start(byte_pos));
            }
            fh.eof_flag = false;
        }
    }
}

// --- extern "C" functions ---

#[unsafe(no_mangle)]
pub extern "C" fn rice_runtime_init() -> *mut RiceRuntime {
    let rt = Box::new(RiceRuntime::new());
    Box::into_raw(rt)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_runtime_shutdown(rt: *mut RiceRuntime) {
    if !rt.is_null() {
        unsafe {
            let mut rt = Box::from_raw(rt);
            let _ = rt.output.flush();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_print(rt: *mut RiceRuntime, tag: i64, data: i64, sep: i32) {
    if rt.is_null() { return; }
    let rt = unsafe { &mut *rt };
    let val = ffi_to_value(tag, data);
    rt.print_value(&val);
    _ = sep;
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_print_comma(rt: *mut RiceRuntime) {
    if rt.is_null() { return; }
    let rt = unsafe { &mut *rt };
    rt.print_comma();
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_print_newline(rt: *mut RiceRuntime) {
    if rt.is_null() { return; }
    let rt = unsafe { &mut *rt };
    rt.print_newline();
}

/// Generic runtime call dispatcher.
#[unsafe(no_mangle)]
pub extern "C" fn rice_runtime_call(
    rt: *mut RiceRuntime,
    name_ptr: *const std::ffi::c_char,
    argc: i32,
    args_ptr: *const i64,
    out_tag: *mut i64,
    out_data: *mut i64,
) {
    if rt.is_null() { return; }
    let rt = unsafe { &mut *rt };

    let name = if name_ptr.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(name_ptr) }.to_str().unwrap_or("")
    };

    let mut args = Vec::new();
    for i in 0..argc as isize {
        let tag = unsafe { *args_ptr.offset(i * 2) };
        let data = unsafe { *args_ptr.offset(i * 2 + 1) };
        args.push((tag, data));
    }

    let (result_tag, result_data) = rt.dispatch(name, &args);
    unsafe {
        *out_tag = result_tag;
        *out_data = result_data;
    }
}
