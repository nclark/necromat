//! Number formatting for the results column.

/// How many decimals the results column shows at most.
pub const MAX_DECIMALS: usize = 5;

/// Formats a value with comma thousands separators and up to
/// [`MAX_DECIMALS`] decimals, trailing zeros trimmed. An integer shows no
/// decimal point. A value that rounds to zero shows as `0` with no sign.
pub fn format_number(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.to_string();
    }

    // Round the whole value, so 1.999999 becomes 2, not 1.99999.
    let fixed = format!("{:.*}", MAX_DECIMALS, v.abs());
    let (int_part, dec) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let dec = dec.trim_end_matches('0');
    let mut body = group_thousands(int_part);
    if !dec.is_empty() {
        body.push('.');
        body.push_str(dec);
    }

    let is_zero = body == "0";
    if v < 0.0 && !is_zero {
        format!("-{body}")
    } else {
        body
    }
}

/// Inserts a comma every three digits from the right.
fn group_thousands(digits: &str) -> String {
    let count = digits.chars().count();
    let mut out = String::with_capacity(count + count / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (count - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_number_table() {
        let cases = [
            (5000.0, "5,000"),
            (1234567.0, "1,234,567"),
            (1234.56, "1,234.56"),
            (-5000.0, "-5,000"),
            (0.5, "0.5"),
            (100.0, "100"),
            (0.0, "0"),
            (-1234567.89, "-1,234,567.89"),
            (999.0, "999"),
            (1000.0, "1,000"),
            (1000000.0, "1,000,000"),
            (1.0 / 3.0, "0.33333"),
            (2.0 / 3.0, "0.66667"),
            (0.004, "0.004"),
            (1234.5, "1,234.5"),
            (1200.50, "1,200.5"),
        ];
        for (input, want) in cases {
            assert_eq!(format_number(input), want, "format_number({input})");
        }
    }

    #[test]
    fn rounds_the_whole_value() {
        assert_eq!(format_number(1.999999), "2");
        assert_eq!(format_number(999.999999), "1,000");
        assert_eq!(format_number(1.999), "1.999");
        assert_eq!(format_number(1400.0000000001), "1,400");
        assert_eq!(format_number(0.000005), "0.00001");
        assert_eq!(format_number(0.0000049), "0");
    }

    #[test]
    fn zero_never_has_a_sign() {
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(-0.000001), "0");
        assert_eq!(format_number(-0.0000049), "0");
        assert_eq!(format_number(-0.00001), "-0.00001");
        assert_eq!(format_number(-0.5), "-0.5");
    }

    #[test]
    fn non_finite_and_huge() {
        assert_eq!(format_number(1e20), "100,000,000,000,000,000,000");
        assert_eq!(format_number(f64::INFINITY), "inf");
        assert_eq!(format_number(f64::NEG_INFINITY), "-inf");
        assert_eq!(format_number(f64::NAN), "NaN");
    }
}
