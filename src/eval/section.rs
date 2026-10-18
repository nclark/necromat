//! The status-bar total for the section under the cursor.

use super::evaluator::LineResult;
use crate::lang::ParsedLine;

fn is_blank(line: &ParsedLine) -> bool {
    line.raw.trim().is_empty()
}

/// Sums the values of the section that contains `cursor_line`.
///
/// A section is a run of non-blank lines plus at most one trailing blank.
/// The cursor on a blank that touches another blank gives 0. Lines that read
/// the accumulator (`used_sum`) are left out, because their value already
/// contains the lines above them and would otherwise be counted twice.
pub fn section_total(results: &[LineResult], lines: &[ParsedLine], cursor_line: usize) -> f64 {
    let n = lines.len();
    if cursor_line >= n {
        return 0.0;
    }
    let blank = |i: usize| lines.get(i).is_some_and(is_blank);

    if blank(cursor_line)
        && ((cursor_line > 0 && blank(cursor_line - 1))
            || (cursor_line + 1 < n && blank(cursor_line + 1)))
    {
        return 0.0;
    }

    // Walk up from the cursor past at most one blank line.
    let Some(pos) = (0..=cursor_line).rev().take(2).find(|&i| !blank(i)) else {
        return 0.0;
    };

    let mut start = pos;
    while start > 0 && !blank(start - 1) {
        start -= 1;
    }
    let mut end = pos;
    while end + 1 < n && !blank(end + 1) {
        end += 1;
    }
    // One trailing blank belongs to the section if it is the last line or
    // is followed by content.
    if end + 1 < n && (end + 2 == n || !blank(end + 2)) {
        end += 1;
    }

    results
        .iter()
        .take(end + 1)
        .skip(start)
        .filter(|r| r.error.is_none() && !r.used_sum)
        .filter_map(|r| r.value)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Evaluator;
    use crate::lang::parse_all;

    fn total(lines: &[&str], cursor: usize) -> f64 {
        let parsed = parse_all(lines);
        let results = Evaluator::new().evaluate_all(&parsed);
        section_total(&results, &parsed, cursor)
    }

    #[test]
    fn calculate_total_table() {
        let two_sections = [
            "# Test One",
            "one=5000",
            "two=3000",
            "",
            "# Test Two",
            "three=10000",
            "four=5000",
        ];
        let cases: Vec<(&str, Vec<&str>, usize, f64)> = vec![
            ("cursor in section 1", two_sections.to_vec(), 1, 8000.0),
            ("cursor in section 2", two_sections.to_vec(), 5, 15000.0),
            (
                "cursor on single trailing blank of section 1",
                two_sections.to_vec(),
                3,
                8000.0,
            ),
            (
                "cursor on multiple consecutive blanks",
                vec![
                    "# Test One",
                    "one=5000",
                    "two=3000",
                    "",
                    "",
                    "# Test Two",
                    "three=10000",
                ],
                3,
                0.0,
            ),
            (
                "cursor on second of multiple blanks",
                vec!["# Test One", "one=5000", "", "", "# Test Two"],
                3,
                0.0,
            ),
            (
                "comment-only section",
                vec!["# Comment only", "", "value=100"],
                0,
                0.0,
            ),
            ("single line section", vec!["value=100"], 0, 100.0),
            (
                "section at end without trailing blank",
                vec!["one=100", "", "two=200", "three=300"],
                3,
                500.0,
            ),
            (
                "section at start",
                vec!["one=100", "two=200", "", "three=300"],
                0,
                300.0,
            ),
            ("empty document", vec![""], 0, 0.0),
            (
                "section with mixed comments and values",
                vec![
                    "# Header",
                    "one=100",
                    "# Middle comment",
                    "two=200",
                    "",
                    "other=999",
                ],
                2,
                300.0,
            ),
        ];
        for (name, lines, cursor, want) in cases {
            assert_eq!(total(&lines, cursor), want, "{name}");
        }
    }

    /// A line that consumed `sum` must not be counted again.
    #[test]
    fn sum_lines_are_not_double_counted() {
        let lines = [
            "90 # Internet",
            "25 # Mint Mobile",
            "85 # Dog food",
            "bills=sum",
        ];
        for cursor in 0..lines.len() {
            assert_eq!(total(&lines, cursor), 200.0, "cursor {cursor}");
        }
        assert_eq!(total(&["100", "200", "sum"], 0), 300.0);
        assert_eq!(total(&["100", "200", "left=500-sum"], 1), 300.0);
    }

    #[test]
    fn errors_are_skipped() {
        assert_eq!(total(&["100", "x/0", "y", "50"], 0), 150.0);
    }

    #[test]
    fn edge_cursors() {
        assert_eq!(total(&[], 0), 0.0);
        assert_eq!(total(&["1"], 5), 0.0);
        // Blank first line followed by content: nothing above it.
        assert_eq!(total(&["", "1"], 0), 0.0);
        // Trailing blank at end of document belongs to the section.
        assert_eq!(total(&["1", "2", ""], 2), 3.0);
        // Whitespace-only lines are blank.
        assert_eq!(total(&["1", "   ", "2"], 2), 2.0);
    }
}
