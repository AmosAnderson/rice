use crate::error::RuntimeError;
use crate::value::Value;

/// Format values according to a PRINT USING format string.
pub fn format_using(format_str: &str, values: &[Value]) -> Result<String, RuntimeError> {
    let mut output = String::new();
    let chars: Vec<char> = format_str.chars().collect();
    let mut fi = 0; // format index
    let mut vi = 0; // value index

    let mut cycle_start_vi = 0;
    loop {
        if fi >= chars.len() {
            if vi >= values.len() {
                break;
            }
            if vi == cycle_start_vi {
                return Err(RuntimeError::IllegalFunctionCall {
                    msg: "PRINT USING format contains no value fields".into(),
                });
            }
            // Values remain — repeat the format string after making progress.
            cycle_start_vi = vi;
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
            let formatted = if !field.scientific
                && let Some(integer) = values[vi].integer_value()
            {
                format_integer(integer, &field)
            } else {
                format_numeric(values[vi].to_f64()?, &field)
            };
            vi += 1;
            output.push_str(&formatted);
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
    digits_after: usize, // digit positions after decimal
    has_comma: bool,
    scientific: bool, // ^^^^
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

    let formatted = format_magnitude(abs_val, field.digits_after);

    format_fixed(&formatted, negative, field)
}

fn format_magnitude(value: f64, places: usize) -> String {
    if !value.is_finite() {
        return value.to_string();
    }

    // Preserve ordinary decimal rounding, including halfway-away-from-zero ties.
    // Large values or precisions need a decimal fallback: neither an infinite
    // scaling factor nor an overflowing product says the value itself is infinite.
    if places <= 308 {
        let factor = 10f64.powi(places as i32);
        let scaled = value * factor;
        if scaled.is_finite() {
            let rounded = scaled.round() / factor;
            return format!("{rounded:.places$}");
        }
        // At this magnitude the f64 has no significant fractional digits beyond
        // the requested precision, so ordinary formatting needs no rounding aid.
        return format!("{value:.places$}");
    }
    round_decimal(exact_decimal(value, 0), places)
}

fn exact_decimal(value: f64, shift: i128) -> String {
    // Every finite f64 has an exact terminating decimal expansion with at most
    // 1074 fractional digits (the denominator of the smallest subnormal is 2^1074).
    let exact = format!("{value:.1074}");
    let point = exact.find('.').unwrap();
    let mut digits = exact.replace('.', "");
    let shifted_point = point as i128 + shift;
    if shifted_point <= 0 {
        digits = format!("0.{}{}", "0".repeat((-shifted_point) as usize), digits);
    } else if shifted_point >= digits.len() as i128 {
        digits.extend(std::iter::repeat_n(
            '0',
            shifted_point as usize - digits.len(),
        ));
    } else {
        digits.insert(shifted_point as usize, '.');
    }
    // Shifting a tiny value can leave leading zeros before its first digit.
    let leading_zeros = digits
        .bytes()
        .take_while(|&ch| ch == b'0')
        .count()
        .min(digits.find('.').unwrap_or(digits.len()).saturating_sub(1));
    digits.drain(..leading_zeros);
    digits
}

fn round_decimal(decimal: String, places: usize) -> String {
    let point = decimal.find('.').unwrap_or(decimal.len());
    let mut digits: Vec<u8> = decimal.bytes().filter(|&ch| ch != b'.').collect();
    let retained = point + places;
    let round_up = digits.get(retained).is_some_and(|&ch| ch >= b'5');
    digits.resize(retained, b'0');
    if round_up {
        let mut cursor = digits.len();
        loop {
            if cursor == 0 {
                digits.insert(0, b'1');
                break;
            }
            cursor -= 1;
            if digits[cursor] != b'9' {
                digits[cursor] += 1;
                break;
            }
            digits[cursor] = b'0';
        }
    }
    if places > 0 {
        digits.insert(digits.len() - places, b'.');
    }
    String::from_utf8(digits).expect("decimal formatting contains only ASCII digits")
}

fn format_integer(value: i64, field: &NumericField) -> String {
    // unsigned_abs also handles i64::MIN without overflow or floating-point conversion.
    let mut formatted = value.unsigned_abs().to_string();
    if field.has_decimal && field.digits_after > 0 {
        formatted.push('.');
        formatted.extend(std::iter::repeat_n('0', field.digits_after));
    }
    format_fixed(&formatted, value < 0, field)
}

fn format_fixed(formatted: &str, negative: bool, field: &NumericField) -> String {
    // Split into integer and fractional parts (including the dot)
    let (int_part, frac_part) = match formatted.find('.') {
        Some(dot) => (&formatted[..dot], &formatted[dot..]),
        None => (formatted, ""),
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
        // With commas: pad positions are also comma-separated,
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

    // Decimal decomposition avoids log10 rounding across powers of ten. Keep the
    // original exponent when rounding widens the mantissa, as the format requires.
    let (man_str, exponent) = if abs_val == 0.0 || !abs_val.is_finite() {
        (format_magnitude(abs_val, field.digits_after), 0i128)
    } else {
        let scientific = format!("{abs_val:e}");
        let (_, exponent) = scientific.split_once('e').unwrap();
        let exponent: i128 = exponent.parse().unwrap();
        let adjusted_exp = exponent - (effective_digits_before as i128 - 1);
        let divisor = i32::try_from(adjusted_exp)
            .ok()
            .map(|exp| 10f64.powi(exp))
            .unwrap_or(0.0);
        let mantissa = abs_val / divisor;
        let formatted = if divisor.is_finite() && divisor != 0.0 && mantissa.is_finite() {
            format_magnitude(mantissa, field.digits_after)
        } else {
            round_decimal(exact_decimal(abs_val, -adjusted_exp), field.digits_after)
        };
        (formatted, adjusted_exp)
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
    } else if negative && !field.trailing_plus && !field.trailing_minus {
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

fn format_exponent(exp: i128) -> String {
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
        if remaining > 0 && remaining.is_multiple_of(3) {
            result.push(',');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_formats_that_cannot_consume_values() {
        for format in ["", "literal text", "_#_!_&", "_"] {
            assert!(matches!(
                format_using(format, &[Value::Numeric(1.0)]),
                Err(RuntimeError::IllegalFunctionCall { .. })
            ));
        }
    }

    #[test]
    fn repeats_fields_and_preserves_literals() {
        assert_eq!(
            format_using("[##]", &[Value::Numeric(1.0), Value::Numeric(2.0)]).unwrap(),
            "[ 1][ 2]",
        );
        assert_eq!(format_using("literal", &[]).unwrap(), "literal");
        assert_eq!(format_using("", &[]).unwrap(), "");
    }

    #[test]
    fn scientific_trailing_sign_is_printed_once() {
        for format in ["##.##^^^^+", "##.##^^^^-"] {
            assert_eq!(
                format_using(format, &[Value::Numeric(-1234.5)]).unwrap(),
                "12.35E+02-",
            );
        }
        assert_eq!(
            format_using("##.##^^^^+", &[Value::Numeric(1234.5)]).unwrap(),
            "12.35E+02+",
        );
    }

    #[test]
    fn fixed_fields_preserve_full_int64_range_and_fractional_zeroes() {
        for (format, value, expected) in [
            ("###################", i64::MAX, "9223372036854775807"),
            ("####################", i64::MIN, "-9223372036854775808"),
            (
                "################.##",
                9_007_199_254_740_993,
                "9007199254740993.00",
            ),
            (
                "####################.##",
                i64::MIN,
                "-9223372036854775808.00",
            ),
            ("#.###", 0, "0.000"),
            ("###.", 42, " 42"),
        ] {
            assert_eq!(
                format_using(format, &[Value::Int64(value)]).unwrap(),
                expected
            );
        }
        assert_eq!(
            format_using("###.##", &[Value::Byte(255)]).unwrap(),
            "255.00"
        );
    }

    #[test]
    fn exact_integer_fields_share_sign_grouping_currency_and_overflow_rules() {
        for (format, value, expected) in [
            ("+###################", i64::MAX, "+9223372036854775807"),
            ("###################+", i64::MIN, "9223372036854775808-"),
            ("###################-", i64::MAX, "9223372036854775807 "),
            (
                "###################,",
                i64::MAX,
                "9,223,372,036,854,775,807",
            ),
            (
                "$$###################.##",
                i64::MAX,
                "$9223372036854775807.00",
            ),
            ("**###################", i64::MIN, "*-9223372036854775808"),
            ("##", i64::MAX, "%9223372036854775807"),
            ("##.##", i64::MIN, "%-9223372036854775808.00"),
            ("###################", i64::MIN, "%-9223372036854775808"),
        ] {
            assert_eq!(
                format_using(format, &[Value::Int64(value)]).unwrap(),
                expected,
                "{format}"
            );
        }
    }

    #[test]
    fn native_integers_preserve_existing_numeric_field_layouts() {
        for format in [
            "###",
            "###.##",
            "###.",
            "+###",
            "###+",
            "###-",
            "##,###.##",
            "**###.##",
            "$$###.##",
            "**$###.##",
        ] {
            for value in [-255, -1, 0, 1, 255] {
                assert_eq!(
                    format_using(format, &[Value::Int64(value)]).unwrap(),
                    format_using(format, &[Value::Numeric(value as f64)]).unwrap(),
                    "{format}, {value}"
                );
            }
            assert_eq!(
                format_using(format, &[Value::Byte(255)]).unwrap(),
                format_using(format, &[Value::Numeric(255.0)]).unwrap(),
                "{format}"
            );
        }
    }

    #[test]
    fn scientific_integer_fields_require_exact_float_conversion() {
        assert!(matches!(
            format_using("#.##^^^^", &[Value::Int64(9_007_199_254_740_993)]),
            Err(RuntimeError::Overflow { .. })
        ));
        assert_eq!(
            format_using("#.##^^^^", &[Value::Int64(1234)]).unwrap(),
            "1.23E+03"
        );
        assert_eq!(
            format_using("#.##^^^^", &[Value::Byte(255)]).unwrap(),
            "2.55E+02"
        );
        for value in [Value::Bool(true), Value::Bytes(vec![42])] {
            assert!(matches!(
                format_using("###", &[value]),
                Err(RuntimeError::TypeMismatch { .. })
            ));
        }
    }

    #[test]
    fn fixed_width_numeric_types_use_their_exact_values() {
        for (format, value, expected) in [
            ("######", Value::Integer(i16::MIN), "-32768"),
            ("###########", Value::Long(i32::MIN), "-2147483648"),
            ("##########.##", Value::Long(i32::MAX), "2147483647.00"),
            ("#.###", Value::Single(0.125), "0.125"),
            ("#.##^^^^", Value::Single(1234.0), "1.23E+03"),
            ("#.##^^^^", Value::Integer(1234), "1.23E+03"),
        ] {
            assert_eq!(format_using(format, &[value]).unwrap(), expected);
        }
    }

    #[test]
    fn fixed_numeric_fields_preserve_finite_extremes() {
        for value in [1e308, f64::MAX] {
            for signed in [value, -value] {
                assert_eq!(
                    format_using("##.##", &[Value::Numeric(signed)]).unwrap(),
                    format!("%{signed:.2}"),
                );
                assert_eq!(
                    format_using("##", &[Value::Numeric(signed)]).unwrap(),
                    format!("%{signed:.0}"),
                );
            }
        }
        assert_eq!(
            format_using("#####################", &[Value::Numeric(1e20)]).unwrap(),
            "100000000000000000000",
        );
        for value in [f64::MIN_POSITIVE, f64::from_bits(1)] {
            assert_eq!(
                format_using("##.##", &[Value::Numeric(value)]).unwrap(),
                " 0.00",
            );
            assert_eq!(
                format_using("##.##", &[Value::Numeric(-value)]).unwrap(),
                "-0.00",
            );
        }
    }

    #[test]
    fn high_precision_fields_round_subnormals_and_exact_ties() {
        let format = format!("+#.{}", "#".repeat(324));
        let expected = format!("0.{}5", "0".repeat(323));
        for (value, sign) in [(f64::from_bits(1), '+'), (-f64::from_bits(1), '-')] {
            assert_eq!(
                format_using(&format, &[Value::Numeric(value)]).unwrap(),
                format!("{sign}{expected}"),
            );
        }

        // 2^-310 terminates at decimal place 310 with ...625, so rounding to
        // place 309 must produce ...63, even though the retained digit is even.
        let tie = 2f64.powi(-310);
        let exact = format!("{tie:.310}");
        assert!(exact.ends_with("625"));
        let expected = format!("{}3", &exact[..exact.len() - 2]);
        assert_eq!(format_magnitude(tie, 309), expected);
        assert_eq!(format_magnitude(tie, 310), exact);
    }

    #[test]
    fn decimal_rounding_keeps_ordinary_ties_and_scientific_width() {
        for (format, value, expected) in [
            ("##", 2.5, " 3"),
            ("##", -2.5, "-3"),
            ("##.##", 1.125, " 1.13"),
            ("##.##", -1.125, "-1.13"),
            ("#.##", 2.675, "2.68"),
            ("#.##^^^^", 9.999, "10.00E+00"),
            ("##.##^^^^", 1234.5, "12.35E+02"),
            ("##.##^^^^", 0.00456, "45.60E-04"),
            ("#.##^^^^", 0.0, "0.00E+00"),
            ("#.##^^^^", -0.0, "0.00E+00"),
        ] {
            assert_eq!(
                format_using(format, &[Value::Numeric(value)]).unwrap(),
                expected,
            );
        }
    }

    #[test]
    fn scientific_fields_cover_the_full_finite_range() {
        for (value, expected) in [
            (f64::MAX, "1.80E+308"),
            (f64::MIN_POSITIVE, "2.23E-308"),
            (1e-309, "1.00E-309"),
            (f64::from_bits(1), "4.94E-324"),
        ] {
            assert_eq!(
                format_using("#.##^^^^", &[Value::Numeric(value)]).unwrap(),
                expected,
            );
            assert_eq!(
                format_using("#.##^^^^", &[Value::Numeric(-value)]).unwrap(),
                format!("-{expected}"),
            );
        }
        assert_eq!(
            format_using("##.##^^^^", &[Value::Numeric(f64::from_bits(1))]).unwrap(),
            "49.41E-325",
        );
        let format = format!("{}.##^^^^", "#".repeat(310));
        assert_eq!(
            format_using(&format, &[Value::Numeric(1.0)]).unwrap(),
            format!("1{}.00E-309", "0".repeat(309)),
        );
    }

    #[test]
    fn nonfinite_values_do_not_enter_decimal_scaling() {
        for (value, expected) in [
            (f64::INFINITY, "infE+00"),
            (f64::NEG_INFINITY, "-infE+00"),
            (f64::NAN, "NaNE+00"),
        ] {
            assert_eq!(
                format_using("#.##^^^^", &[Value::Numeric(value)]).unwrap(),
                expected,
            );
            assert!(
                !format_using("###.##", &[Value::Numeric(value)])
                    .unwrap()
                    .is_empty()
            );
        }
    }
}
