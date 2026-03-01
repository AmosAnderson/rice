use crate::error::RuntimeError;
use crate::value::Value;

/// Format values according to a QBasic PRINT USING format string.
pub fn format_using(format_str: &str, values: &[Value]) -> Result<String, RuntimeError> {
    let mut output = String::new();
    let chars: Vec<char> = format_str.chars().collect();
    let mut fi = 0; // format index
    let mut vi = 0; // value index

    loop {
        if fi >= chars.len() {
            if vi >= values.len() {
                break;
            }
            // Values remain — repeat the format string
            fi = 0;
        }

        let ch = chars[fi];

        // Escape: _ means next char is literal
        if ch == '_' {
            fi += 1;
            if fi < chars.len() {
                output.push(chars[fi]);
                fi += 1;
            }
            continue;
        }

        // String format: !
        if ch == '!' {
            if vi >= values.len() {
                break;
            }
            let s = values[vi].to_string_val()?;
            vi += 1;
            output.push(s.chars().next().unwrap_or(' '));
            fi += 1;
            continue;
        }

        // String format: & (entire string)
        if ch == '&' {
            if vi >= values.len() {
                break;
            }
            let s = values[vi].to_string_val()?;
            vi += 1;
            output.push_str(&s);
            fi += 1;
            continue;
        }

        // String format: \ spaces \ (fixed width)
        if ch == '\\' {
            let start = fi;
            fi += 1;
            while fi < chars.len() && chars[fi] != '\\' {
                fi += 1;
            }
            if fi < chars.len() {
                fi += 1; // consume closing backslash
            }
            let width = fi - start; // includes both backslashes
            if vi >= values.len() {
                break;
            }
            let s = values[vi].to_string_val()?;
            vi += 1;
            let s_chars: Vec<char> = s.chars().collect();
            if s_chars.len() >= width {
                for &c in &s_chars[..width] {
                    output.push(c);
                }
            } else {
                output.push_str(&s);
                for _ in 0..(width - s_chars.len()) {
                    output.push(' ');
                }
            }
            continue;
        }

        // Numeric format: starts with #, +, ., *, or $$
        if is_numeric_format_start(ch, &chars, fi) {
            if vi >= values.len() {
                break;
            }
            let (field, new_fi) = parse_numeric_field(&chars, fi);
            fi = new_fi;
            let val = values[vi].to_f64()?;
            vi += 1;
            output.push_str(&format_numeric(val, &field));
            continue;
        }

        // Literal character
        output.push(ch);
        fi += 1;
    }

    Ok(output)
}

fn is_numeric_format_start(ch: char, chars: &[char], pos: usize) -> bool {
    match ch {
        '#' | '.' => true,
        '+' => pos + 1 < chars.len() && matches!(chars[pos + 1], '#' | '.' | '+' | '$' | '*'),
        '*' => pos + 1 < chars.len() && chars[pos + 1] == '*',
        '$' => pos + 1 < chars.len() && chars[pos + 1] == '$',
        _ => false,
    }
}

#[derive(Debug, Default)]
struct NumericField {
    leading_plus: bool,
    trailing_plus: bool,
    trailing_minus: bool,
    dollar_float: bool,    // $$
    asterisk_fill: bool,   // **
    asterisk_dollar: bool, // **$
    digits_before: usize,  // digit positions before decimal
    has_decimal: bool,
    digits_after: usize,   // digit positions after decimal
    has_comma: bool,
    scientific: bool,      // ^^^^
}

fn parse_numeric_field(chars: &[char], start: usize) -> (NumericField, usize) {
    let mut field = NumericField::default();
    let mut pos = start;

    // Leading +
    if pos < chars.len() && chars[pos] == '+' {
        field.leading_plus = true;
        pos += 1;
    }

    // ** or **$
    if pos + 1 < chars.len() && chars[pos] == '*' && chars[pos + 1] == '*' {
        field.asterisk_fill = true;
        pos += 2;
        field.digits_before += 2;
        if pos < chars.len() && chars[pos] == '$' {
            field.asterisk_dollar = true;
            pos += 1;
        }
    }
    // $$
    else if pos + 1 < chars.len() && chars[pos] == '$' && chars[pos + 1] == '$' {
        field.dollar_float = true;
        pos += 2;
        field.digits_before += 1; // $$ provides one digit position
    }

    // # and , before decimal
    while pos < chars.len() {
        match chars[pos] {
            '#' => {
                field.digits_before += 1;
                pos += 1;
            }
            ',' => {
                if !field.has_decimal {
                    field.has_comma = true;
                    pos += 1;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    // Decimal point
    if pos < chars.len() && chars[pos] == '.' {
        field.has_decimal = true;
        pos += 1;
        while pos < chars.len() && chars[pos] == '#' {
            field.digits_after += 1;
            pos += 1;
        }
    }

    // ^^^^
    if pos + 3 < chars.len()
        && chars[pos] == '^'
        && chars[pos + 1] == '^'
        && chars[pos + 2] == '^'
        && chars[pos + 3] == '^'
    {
        field.scientific = true;
        pos += 4;
    }

    // Trailing + or -
    if pos < chars.len() && chars[pos] == '+' && !field.leading_plus {
        field.trailing_plus = true;
        pos += 1;
    } else if pos < chars.len() && chars[pos] == '-' {
        field.trailing_minus = true;
        pos += 1;
    }

    (field, pos)
}

fn format_numeric(value: f64, field: &NumericField) -> String {
    if field.scientific {
        return format_scientific(value, field);
    }

    let negative = value < 0.0;
    let abs_val = value.abs();

    // Round to the specified decimal places
    let rounded = if field.has_decimal {
        let factor = 10f64.powi(field.digits_after as i32);
        (abs_val * factor).round() / factor
    } else {
        abs_val.round()
    };

    // Format the absolute value
    let formatted = if field.has_decimal {
        format!("{:.prec$}", rounded, prec = field.digits_after)
    } else {
        format!("{}", rounded as i64)
    };

    // Split into integer and fractional parts (including the dot)
    let (int_part, frac_part) = match formatted.find('.') {
        Some(dot) => (&formatted[..dot], &formatted[dot..]),
        None => (formatted.as_str(), ""),
    };

    // Apply thousands separator
    let int_str = if field.has_comma {
        add_thousands_separator(int_part)
    } else {
        int_part.to_string()
    };

    // Check overflow: integer part wider than available digit positions
    // When comma formatting is active, the formatted integer will be wider,
    // but we compare against raw digit count (without commas)
    let raw_int_len = int_part.len();
    if raw_int_len > field.digits_before {
        let sign = if negative { "-" } else { "" };
        return format!("%{}{}{}", sign, int_str, frac_part);
    }

    // Build the body: right-aligned integer + fractional
    let fill_char = if field.asterisk_fill { '*' } else { ' ' };

    // When comma formatting is active, the display width includes commas,
    // but padding is based on raw digit positions vs raw digit count
    let pad_count = field.digits_before - raw_int_len;

    let mut padded = String::new();
    if field.has_comma {
        // With commas: pad positions are also comma-separated in QBasic,
        // so we pad based on displayed width including commas
        let display_int_width = int_str.len();
        // Total display width for integer portion = digits_before + (digits_before-1)/3 commas
        // But only if there are enough digits to have commas
        let total_int_display = if field.digits_before > 3 {
            field.digits_before + (field.digits_before - 1) / 3
        } else {
            field.digits_before
        };
        let display_pad = total_int_display.saturating_sub(display_int_width);
        for _ in 0..display_pad {
            padded.push(fill_char);
        }
    } else {
        for _ in 0..pad_count {
            padded.push(fill_char);
        }
    }
    padded.push_str(&int_str);
    padded.push_str(frac_part);

    // Place the sign
    if field.leading_plus {
        // Sign goes at the very front, outside the digit area
        let sign = if negative { '-' } else { '+' };
        padded = format!("{}{}", sign, padded);
    } else if negative && !field.trailing_plus && !field.trailing_minus {
        // Embed sign: replace the rightmost fill/space before digits with '-'
        let chars_vec: Vec<char> = padded.chars().collect();
        let mut sign_pos: Option<usize> = None;
        for (i, c) in chars_vec.iter().enumerate() {
            if *c == fill_char {
                sign_pos = Some(i);
            } else {
                break;
            }
        }
        if let Some(pos) = sign_pos {
            let mut chars_mut = chars_vec;
            chars_mut[pos] = '-';
            padded = chars_mut.into_iter().collect();
        } else {
            // No fill space available — overflow
            return format!("%-{}{}", int_str, frac_part);
        }
    }

    // Float the dollar sign adjacent to the number
    if field.dollar_float || field.asterisk_dollar {
        let chars_vec: Vec<char> = padded.chars().collect();
        // Find position just before first digit (or '-')
        let first_digit = chars_vec
            .iter()
            .position(|c| c.is_ascii_digit() || *c == '-');
        match first_digit {
            Some(pos) if pos > 0 => {
                let mut chars_mut = chars_vec;
                chars_mut[pos - 1] = '$';
                padded = chars_mut.into_iter().collect();
            }
            _ => {
                padded = format!("${}", padded);
            }
        }
    }

    // Trailing sign
    if field.trailing_plus {
        padded.push(if negative { '-' } else { '+' });
    } else if field.trailing_minus {
        padded.push(if negative { '-' } else { ' ' });
    }

    padded
}

fn format_scientific(value: f64, field: &NumericField) -> String {
    let negative = value < 0.0;
    let abs_val = value.abs();

    let effective_digits_before = field.digits_before.max(1);

    // Compute mantissa and exponent
    // For digits_before=2 and value=1234.5: exp=3, adjusted_exp=2, man=12.345
    let (mantissa, exponent) = if abs_val == 0.0 {
        (0.0, 0i32)
    } else {
        let exp = abs_val.log10().floor() as i32;
        let adjusted_exp = exp - (effective_digits_before as i32 - 1);
        let man = abs_val / 10f64.powi(adjusted_exp);
        (man, adjusted_exp)
    };

    // Round mantissa
    let factor = 10f64.powi(field.digits_after as i32);
    let rounded_man = (mantissa * factor).round() / factor;

    let man_str = if field.has_decimal {
        format!("{:.prec$}", rounded_man, prec = field.digits_after)
    } else {
        format!("{}", rounded_man as i64)
    };

    // Pad the mantissa part
    let int_part_len = man_str.find('.').unwrap_or(man_str.len());
    let padding = effective_digits_before.saturating_sub(int_part_len);
    let fill_char = if field.asterisk_fill { '*' } else { ' ' };

    let mut result = String::new();

    // Sign handling
    if field.leading_plus {
        result.push(if negative { '-' } else { '+' });
        for _ in 0..padding {
            result.push(fill_char);
        }
    } else if negative {
        // Place '-' just before digits, after any padding
        for _ in 0..padding.saturating_sub(1) {
            result.push(fill_char);
        }
        result.push('-');
    } else {
        for _ in 0..padding {
            result.push(fill_char);
        }
    }

    result.push_str(&man_str);
    result.push_str(&format_exponent(exponent));

    // Trailing sign
    if field.trailing_plus {
        result.push(if negative { '-' } else { '+' });
    } else if field.trailing_minus {
        result.push(if negative { '-' } else { ' ' });
    }

    result
}

fn format_exponent(exp: i32) -> String {
    if exp >= 0 {
        format!("E+{:02}", exp)
    } else {
        format!("E-{:02}", exp.abs())
    }
}

fn add_thousands_separator(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    let len = chars.len();
    for (i, ch) in chars.iter().enumerate() {
        result.push(*ch);
        let remaining = len - i - 1;
        if remaining > 0 && remaining % 3 == 0 {
            result.push(',');
        }
    }
    result
}
