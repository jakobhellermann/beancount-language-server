//! Beancount file sorter - sorts dated directives by date while preserving blank line spacing.
//!
//! # Algorithm Overview
//!
//! ## Goal
//! Sort dated directives (transactions, balance assertions, etc.) by date within sortable groups,
//! while preserving blank lines that existed in the original file.
//!
//! ## Process
//!
//! 1. **Extract dated directives**: Find all directives that have dates using tree-sitter queries.
//!
//! 2. **Compute spacing information for each directive**:
//!    - `trailing_blank_lines`: Number of blank lines after this directive in the original file
//!    - `leading_blank_lines`: Number of blank lines before this directive in the original file
//!    - `blank_lines`: "Characteristic spacing" representing how much spacing this directive
//!      intrinsically wants:
//!      - First directive: uses trailing (no leading context)
//!      - Last directive: uses leading (no trailing context)
//!      - Middle directives: uses `min(trailing, leading)` - only preserves spacing if it's
//!        consistent on BOTH sides
//!
//! 3. **Identify sortable groups**: Split directives into groups separated by boundaries
//!    (comments, non-dated directives like `option`, `include`, `plugin`, etc.). Each group
//!    is sorted independently.
//!
//! 4. **Sort each group by date** (stable sort, preserving order for same-date directives).
//!
//! 5. **Reconstruct the file** with spacing:
//!    - For each pair of consecutive directives in the sorted output:
//!      - If they were consecutive in the original file (based on `original_index`):
//!        Use `current.trailing_blank_lines` - preserves the exact original spacing
//!      - If they were NOT consecutive in the original file:
//!        Use `max(current.blank_lines, next.blank_lines)` - uses the characteristic spacing
//!        of whichever directive "wants" more space
//!
//! ## Example
//!
//! Original file:
//! ```beancount
//! 2020-01-03 * "C"
//!
//! 2020-01-02 * "B"
//! 2020-01-01 * "A"
//! ```
//!
//! - Directive C: trailing=1, leading=0, blank_lines=1 (first)
//! - Directive B: trailing=0, leading=1, blank_lines=min(0,1)=0 (middle)
//! - Directive A: trailing=0, leading=0, blank_lines=0 (last)
//!
//! After sorting to A-B-C:
//! - A→B spacing: B and A were consecutive (indices 2→1), use A.trailing=0 → no blank
//! - B→C spacing: C and B were consecutive (indices 0→1), use B.trailing=0 → no blank
//!   BUT they weren't consecutive in new order, so use max(B.blank=0, C.blank=1)=1 → one blank
//!
//! Result:
//! ```beancount
//! 2020-01-01 * "A"
//! 2020-01-02 * "B"
//!
//! 2020-01-03 * "C"
//! ```
//!
//! The blank line that was originally between C and B is preserved between B and C.

use std::collections::HashMap;
use std::ops::RangeInclusive;

use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};
use tree_sitter_beancount::language;
use tree_sitter_beancount::tree_sitter;
use tree_sitter_beancount::tree_sitter::Language;
use tree_sitter_beancount::tree_sitter::Tree;

pub struct Formatter {
    language: Language,
    dated_directive_query: Query,
    boundary_query: Query,
}

impl Formatter {
    pub fn new() -> Self {
        let language = language();
        let dated_directive_query = Query::new(&language, r#"(_ date: (date) @date) @directive"#)
            .expect("Failed to create dated directive query");
        let boundary_query = Query::new(&language, r#"(_ !date) @directive"#)
            .expect("Failed to create boundary query");

        Self {
            language,
            dated_directive_query,
            boundary_query,
        }
    }

    pub fn sort(&self, content: &str) -> String {
        let mut parser = Parser::new();
        parser.set_language(&self.language).unwrap();
        let tree = parser.parse(content, None).unwrap();
        self.sort_tree_by_date(content, tree)
    }

    fn sort_tree_by_date(&self, content: &str, tree: Tree) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let directives = self.extract_dated_directives(&tree, content, &lines);

        if directives.is_empty() {
            return content.to_string();
        }

        let groups = self.identify_sortable_groups(&directives, &tree, content);

        if groups.iter().all(|g| is_sorted(g)) {
            return content.to_string();
        }

        reconstruct_file(content, &groups)
    }

    /// Extract all directives that have dates
    fn extract_dated_directives(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        lines: &[&str],
    ) -> Vec<DatedDirective> {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(
            &self.dated_directive_query,
            tree.root_node(),
            content.as_bytes(),
        );

        let mut directives = Vec::new();

        while let Some(match_) = matches.next() {
            let mut date_node = None;
            let mut directive_node = None;

            for capture in match_.captures {
                let capture_name =
                    self.dated_directive_query.capture_names()[capture.index as usize];
                match capture_name {
                    "date" => date_node = Some(capture.node),
                    "directive" => directive_node = Some(capture.node),
                    _ => {}
                }
            }

            if let (Some(date), Some(directive)) = (date_node, directive_node) {
                let date_text = &content[date.start_byte()..date.end_byte()];
                let start_line = directive.start_position().row;
                let mut end_line = directive.end_position().row;

                // tree-sitter end_position with column=0 means "start of that line"
                if directive.end_position().column == 0 && end_line > 0 {
                    end_line -= 1;
                }

                let text = lines[start_line..=end_line].join("\n");

                directives.push(DatedDirective {
                    lines: start_line..=end_line,
                    date: date_text.to_string(),
                    text,
                    blank_lines: 0,
                    trailing_blank_lines: 0,
                    leading_blank_lines: 0,
                    original_index: directives.len(),
                });
            }
        }

        directives.sort_by_key(|d| *d.lines.start());

        for i in 0..directives.len() {
            let end_line = *directives[i].lines.end();
            let start_line = *directives[i].lines.start();

            // Compute trailing blank lines
            let next_start = if i + 1 < directives.len() {
                *directives[i + 1].lines.start()
            } else {
                lines.len()
            };
            let trailing = (end_line + 1..next_start)
                .take_while(|&j| lines[j].trim().is_empty())
                .count();

            // Compute leading blank lines
            let prev_end = if i > 0 {
                *directives[i - 1].lines.end()
            } else {
                0
            };
            let leading = if i > 0 {
                (prev_end + 1..start_line)
                    .take_while(|&j| lines[j].trim().is_empty())
                    .count()
            } else {
                0 // First directive has no leading blanks
            };

            // Store actual trailing and leading blanks
            directives[i].trailing_blank_lines = trailing;
            directives[i].leading_blank_lines = leading;

            // Compute characteristic blank_lines: spacing that exists consistently
            // around this directive (not just on one side)
            directives[i].blank_lines = if i == 0 {
                trailing // First: use trailing only
            } else if i == directives.len() - 1 {
                leading // Last: use leading only
            } else {
                // Middle: use min - only preserve spacing if it's consistent on both sides
                trailing.min(leading)
            };
        }

        directives.sort_by_key(|d| d.original_index);

        directives
    }

    /// Identify groups of directives that can be sorted together
    fn identify_sortable_groups(
        &self,
        directives: &[DatedDirective],
        tree: &tree_sitter::Tree,
        content: &str,
    ) -> Vec<SortableGroup> {
        if directives.is_empty() {
            return vec![];
        }

        let boundaries = self.find_boundaries(tree, content);

        let mut groups = Vec::new();
        let mut current_group: Vec<DatedDirective> = Vec::new();

        for directive in directives {
            let has_boundary = if let Some(last) = current_group.last() {
                boundaries
                    .iter()
                    .any(|&b| b > *last.lines.end() && b < *directive.lines.start())
            } else {
                false
            };

            if has_boundary && !current_group.is_empty() {
                groups.push(SortableGroup {
                    directives: current_group,
                });
                current_group = Vec::new();
            }

            current_group.push(directive.clone());
        }

        if !current_group.is_empty() {
            groups.push(SortableGroup {
                directives: current_group,
            });
        }

        groups
    }

    /// Find all boundary lines
    fn find_boundaries(&self, tree: &tree_sitter::Tree, content: &str) -> Vec<usize> {
        let mut boundaries = Vec::new();

        let mut cursor = QueryCursor::new();
        let mut matches =
            cursor.matches(&self.boundary_query, tree.root_node(), content.as_bytes());
        while let Some(match_) = matches.next() {
            let [capture] = match_.captures else {
                unreachable!();
            };
            boundaries.push(capture.node.start_position().row);
        }

        boundaries
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct DatedDirective {
    lines: RangeInclusive<usize>, // 0-indexed
    date: String,                 // YYYY-MM-DD format
    text: String,                 // Full directive text
    blank_lines: usize, // Characteristic spacing: min(leading, trailing) with special handling
    trailing_blank_lines: usize, // Actual trailing blanks in original (for skipping lines)
    leading_blank_lines: usize, // Actual leading blanks in original (for reconstruction)
    original_index: usize, // For stable sorting
}

#[derive(Debug)]
struct SortableGroup {
    directives: Vec<DatedDirective>,
}

/// Sort a beancount file by date within groups separated by comments or non-dated directives.
pub fn sort(content: &str) -> String {
    let formatter = Formatter::new();
    formatter.sort(content)
}

fn is_sorted(group: &SortableGroup) -> bool {
    group.directives.is_sorted_by(|a, b| a.date < b.date)
}
fn reconstruct_file(content: &str, groups: &[SortableGroup]) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();

    let sorted_groups: Vec<Vec<DatedDirective>> = groups
        .iter()
        .map(|group| {
            let mut sorted = group.directives.clone();
            sorted.sort_by(|a, b| {
                a.date
                    .cmp(&b.date)
                    .then(a.original_index.cmp(&b.original_index))
            });
            sorted
        })
        .collect();

    let mut directive_lines = std::collections::HashSet::new();
    for group in groups {
        for (i, directive) in group.directives.iter().enumerate() {
            for line in directive.lines.clone() {
                directive_lines.insert(line);
            }
            // Mark actual trailing blank lines for skipping, but not for the last directive in the group
            // (those trailing blanks are spacing before the next boundary and should be preserved)
            if i < group.directives.len() - 1 {
                for j in 1..=directive.trailing_blank_lines {
                    directive_lines.insert(*directive.lines.end() + j);
                }
            }
        }
    }

    let mut group_start_map: HashMap<usize, Vec<String>> = HashMap::new();
    for (group, sorted_directives) in groups.iter().zip(sorted_groups.iter()) {
        if let Some(first_directive) = group.directives.first() {
            let mut sorted_texts = Vec::new();
            for (i, d) in sorted_directives.iter().enumerate() {
                sorted_texts.push(d.text.clone());
                if i < sorted_directives.len() - 1 {
                    let current = &sorted_directives[i];
                    let next = &sorted_directives[i + 1];

                    // Check if current and next were originally consecutive
                    let were_consecutive = current.original_index + 1 == next.original_index;

                    let separation = if were_consecutive {
                        // They were consecutive in original, use current's trailing
                        current.trailing_blank_lines
                    } else {
                        // They weren't consecutive, use max of their characteristic spacing
                        current.blank_lines.max(next.blank_lines)
                    };

                    for _ in 0..separation {
                        sorted_texts.push(String::new());
                    }
                }
            }
            group_start_map.insert(*first_directive.lines.start(), sorted_texts);
        }
    }

    let mut line_idx = 0;
    while line_idx < lines.len() {
        if let Some(sorted_texts) = group_start_map.get(&line_idx) {
            result.extend(sorted_texts.iter().cloned());
            while line_idx < lines.len() && directive_lines.contains(&line_idx) {
                line_idx += 1;
            }
        } else if directive_lines.contains(&line_idx) {
            line_idx += 1;
        } else {
            result.push(lines[line_idx].to_string());
            line_idx += 1;
        }
    }

    let mut output = result.join("\n");

    if content.ends_with('\n') && !output.ends_with('\n') {
        output.push('\n');
    }
    if !content.starts_with('\n') {
        output = output.trim_start_matches('\n').to_string();
    }

    output
}

#[cfg(test)]
mod tests;
