//! Line diff between two results.
//!
//! A mutating tool changes something, and the only way to *see* that from a
//! native window is to compare what a read returned before and after. GPUI
//! cannot host the page, but it already keeps every run — so the change is
//! right here, in data we have.
//!
//! Plain LCS over lines. Inputs are capped, so the quadratic table stays small.

/// Longest pair of results we will compare. Beyond this the diff stops being
/// readable anyway, and the table stops being cheap.
pub const MAX_LINES: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Same,
    Added,
    Removed,
}

impl Change {
    pub fn marker(self) -> &'static str {
        match self {
            Self::Same => " ",
            Self::Added => "+",
            Self::Removed => "−",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub change: Change,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Tally {
    pub added: usize,
    pub removed: usize,
}

impl Tally {
    pub fn is_unchanged(self) -> bool {
        self.added == 0 && self.removed == 0
    }

    /// What the header says, in words rather than symbols.
    pub fn summary(self) -> String {
        if self.is_unchanged() {
            return "No change".to_string();
        }
        let mut parts = Vec::new();
        if self.added > 0 {
            parts.push(format!(
                "{} line{} added",
                self.added,
                if self.added == 1 { "" } else { "s" }
            ));
        }
        if self.removed > 0 {
            parts.push(format!(
                "{} line{} removed",
                self.removed,
                if self.removed == 1 { "" } else { "s" }
            ));
        }
        parts.join(", ")
    }
}

pub fn tally(lines: &[Line]) -> Tally {
    let mut out = Tally::default();
    for line in lines {
        match line.change {
            Change::Added => out.added += 1,
            Change::Removed => out.removed += 1,
            Change::Same => {}
        }
    }
    out
}

fn split(text: &str) -> Vec<String> {
    text.lines().take(MAX_LINES).map(str::to_string).collect()
}

/// Compare two pretty-printed results, oldest first.
pub fn compare(before: &str, after: &str) -> Vec<Line> {
    let before = split(before);
    let after = split(after);
    let (n, m) = (before.len(), after.len());

    // dp[i][j] = length of the longest common subsequence of the tails.
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if before[i] == after[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if before[i] == after[j] {
            out.push(Line { change: Change::Same, text: before[i].clone() });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push(Line { change: Change::Removed, text: before[i].clone() });
            i += 1;
        } else {
            out.push(Line { change: Change::Added, text: after[j].clone() });
            j += 1;
        }
    }
    while i < n {
        out.push(Line { change: Change::Removed, text: before[i].clone() });
        i += 1;
    }
    while j < m {
        out.push(Line { change: Change::Added, text: after[j].clone() });
        j += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            // Unchanged lines keep a blank gutter so the columns line up.
            .map(|line| format!("{}{}", line.change.marker(), line.text))
            .collect()
    }

    #[test]
    fn identical_results_report_no_change_at_all() {
        let text = "{\n  \"a\": 1\n}";
        let lines = compare(text, text);
        assert!(lines.iter().all(|line| line.change == Change::Same));
        assert!(tally(&lines).is_unchanged());
        assert_eq!(tally(&lines).summary(), "No change");
    }

    #[test]
    fn a_changed_value_shows_as_one_out_and_one_in() {
        // What you see after a mutating tool ran between two reads.
        let before = "{\n  \"notes\": 0\n}";
        let after = "{\n  \"notes\": 1\n}";
        assert_eq!(
            rendered(&compare(before, after)),
            vec![" {", "−  \"notes\": 0", "+  \"notes\": 1", " }"]
        );
        let counted = tally(&compare(before, after));
        assert_eq!(counted.summary(), "1 line added, 1 line removed");
    }

    #[test]
    fn added_lines_keep_the_surrounding_context() {
        let before = "a\nb\nc";
        let after = "a\nb\nnew\nc";
        assert_eq!(rendered(&compare(before, after)), vec![" a", " b", "+new", " c"]);
        assert_eq!(tally(&compare(before, after)).summary(), "1 line added");
    }

    #[test]
    fn removed_lines_are_marked_not_dropped() {
        let before = "a\ngone\nb";
        let after = "a\nb";
        assert_eq!(rendered(&compare(before, after)), vec![" a", "−gone", " b"]);
        assert_eq!(tally(&compare(before, after)).summary(), "1 line removed");
    }

    #[test]
    fn comparing_against_nothing_reports_everything_as_new() {
        let lines = compare("", "a\nb");
        assert!(lines.iter().all(|line| line.change == Change::Added));
        assert_eq!(tally(&lines).added, 2);
    }

    #[test]
    fn the_comparison_is_bounded_however_big_the_results_are() {
        let huge: String = (0..5_000).map(|n| format!("{n}\n")).collect();
        let other: String = (0..5_000).map(|n| format!("x{n}\n")).collect();
        let lines = compare(&huge, &other);
        // Both sides are capped, so the output cannot exceed twice the cap.
        assert!(lines.len() <= MAX_LINES * 2, "{} lines", lines.len());
    }

    #[test]
    fn every_change_has_a_distinct_marker() {
        let mut markers: Vec<&str> =
            [Change::Same, Change::Added, Change::Removed].map(Change::marker).to_vec();
        markers.sort_unstable();
        markers.dedup();
        assert_eq!(markers.len(), 3);
    }

    #[test]
    fn a_reordered_result_is_not_reported_as_unchanged() {
        let lines = compare("a\nb", "b\na");
        assert!(!tally(&lines).is_unchanged());
    }
}
