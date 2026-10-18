//! Number literal rules.
//!
//! A number matches `^-?([0-9]{1,3}(,[0-9]{3})+|[0-9]+)(\.[0-9]+)?$`: an
//! optional minus, an integer part that either has no commas or groups digits
//! in threes after a lead group of one to three digits, and an optional
//! fraction of one or more digits.

/// Reports whether `s` is a number literal with valid comma grouping.
pub fn is_valid_number(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    let (int_part, frac_part) = match s.split_once('.') {
        Some((int_part, frac)) => (int_part, Some(frac)),
        None => (s, None),
    };
    if let Some(frac) = frac_part
        && !all_digits(frac)
    {
        return false;
    }
    if !int_part.contains(',') {
        return all_digits(int_part);
    }
    let mut groups = int_part.split(',');
    let lead_ok = groups
        .next()
        .is_some_and(|g| (1..=3).contains(&g.chars().count()) && all_digits(g));
    lead_ok && groups.all(|g| g.chars().count() == 3 && all_digits(g))
}

/// Parses a number literal that already passed [`is_valid_number`]. Commas
/// are dropped. Returns `None` if the value does not fit in a finite `f64`.
pub fn parse_number(s: &str) -> Option<f64> {
    let cleaned: String = s.chars().filter(|&c| c != ',').collect();
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// True when `s` is non-empty and every character is an ASCII digit.
fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_numbers() {
        for s in [
            "0",
            "100",
            "-50",
            "3.14",
            "-3.14",
            "5,000",
            "-5,000",
            "1,234,567.89",
            "-1,234.56",
            "1234",
            "12,345",
            "123,456",
            "1,234,567",
        ] {
            assert!(is_valid_number(s), "{s} should be valid");
        }
    }

    #[test]
    fn invalid_numbers() {
        for s in [
            "", "-", ".5", "5.", "1.2.3", "1,23,4", "12,34", "1234,567", ",123", "123,", "1,,234",
            "1,2345", "-1,23,4", "1.2,3", "--1", "1a", "١٢٣",
        ] {
            assert!(!is_valid_number(s), "{s} should be invalid");
        }
    }

    #[test]
    fn parse_drops_commas() {
        assert_eq!(parse_number("5,000"), Some(5000.0));
        assert_eq!(parse_number("-1,234.56"), Some(-1234.56));
        assert_eq!(parse_number(&"9".repeat(400)), None);
    }
}
