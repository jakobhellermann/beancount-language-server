use crate::server::LspServerStateSnapshot;
use crate::treesitter_utils::text_for_tree_sitter_node;
use crate::utils::ToFilePath;
use anyhow::Result;
use std::collections::HashMap;
use tree_sitter_beancount::tree_sitter;

/// Main handler for code actions
pub(crate) fn code_actions(
    snapshot: LspServerStateSnapshot,
    params: lsp_types::CodeActionParams,
) -> Result<Option<Vec<lsp_types::CodeActionOrCommand>>> {
    let path = match params.text_document.uri.to_file_path() {
        Ok(path) => path,
        Err(_) => {
            tracing::debug!("Failed to convert URI to file path");
            return Ok(None);
        }
    };

    // Get the tree and content for this file
    let tree = match snapshot.forest.get(&path) {
        Some(tree) => tree,
        None => {
            tracing::debug!("No parse tree found for file");
            return Ok(None);
        }
    };

    let content = match snapshot.open_docs.get(&path) {
        Some(doc) => &doc.content,
        None => {
            tracing::debug!("Document not open");
            return Ok(None);
        }
    };

    // Try to find narration at cursor and build the code action
    match find_narration_at_cursor(tree, content, params.range.start) {
        Some(narration_info) => {
            if let Some(action) =
                build_move_to_metadata_action(content, &narration_info, &params.text_document.uri)
            {
                Ok(Some(vec![lsp_types::CodeActionOrCommand::CodeAction(
                    action,
                )]))
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}

#[derive(Debug)]
struct NarrationInfo {
    /// The tree-sitter node for the narration
    narration_node: tree_sitter::Node<'static>,
    /// The parent transaction node
    transaction_node: tree_sitter::Node<'static>,
    /// The narration text (without quotes)
    narration_text: String,
}

/// Find narration node at the given cursor position
fn find_narration_at_cursor(
    tree: &tree_sitter::Tree,
    content: &ropey::Rope,
    cursor_position: lsp_types::Position,
) -> Option<NarrationInfo> {
    // Convert LSP position to tree-sitter Point
    let ts_point = tree_sitter::Point {
        row: cursor_position.line as usize,
        column: cursor_position.character as usize,
    };

    // Find the node at the cursor position
    let root = tree.root_node();
    let node = root.named_descendant_for_point_range(ts_point, ts_point)?;

    // Check if we're on a narration node
    let (narration_node, transaction_node) = if node.kind() == "narration" {
        // Direct hit on narration
        let transaction = find_parent_transaction(&node)?;
        (node, transaction)
    } else if node.kind() == "string" {
        // We might be on the string content inside narration
        let parent = node.parent()?;
        if parent.kind() == "narration" {
            let transaction = find_parent_transaction(&parent)?;
            (parent, transaction)
        } else {
            return None;
        }
    } else {
        return None;
    };

    // Extract narration text
    let narration_text_raw = text_for_tree_sitter_node(content, &narration_node);
    let narration_text = narration_text_raw.trim().trim_matches('"').to_string();

    // Skip if narration is empty or whitespace-only
    if narration_text.trim().is_empty() {
        return None;
    }

    // Check if source_desc already exists
    if has_source_desc_metadata(&transaction_node, content) {
        tracing::debug!("Transaction already has source_desc metadata");
        return None;
    }

    // Make the node 'static by leaking the tree
    // This is safe because we only use it within this function scope
    let narration_node_static = unsafe { std::mem::transmute(narration_node) };
    let transaction_node_static = unsafe { std::mem::transmute(transaction_node) };

    Some(NarrationInfo {
        narration_node: narration_node_static,
        transaction_node: transaction_node_static,
        narration_text,
    })
}

/// Find the parent transaction node
fn find_parent_transaction<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    let mut current = *node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "transaction" {
            return Some(parent);
        }
        current = parent;
    }
    None
}

/// Check if the transaction already has source_desc metadata
fn has_source_desc_metadata(transaction_node: &tree_sitter::Node, content: &ropey::Rope) -> bool {
    let mut cursor = transaction_node.walk();
    for child in transaction_node.children(&mut cursor) {
        if child.kind() == "key_value" {
            // Check if the key is "source_desc"
            let mut kv_cursor = child.walk();
            for kv_child in child.children(&mut kv_cursor) {
                if kv_child.kind() == "key" {
                    let key_text = text_for_tree_sitter_node(content, &kv_child);
                    if key_text == "source_desc" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Build the code action to move narration to metadata
fn build_move_to_metadata_action(
    content: &ropey::Rope,
    narration_info: &NarrationInfo,
    uri: &lsp_types::Uri,
) -> Option<lsp_types::CodeAction> {
    // Build the two text edits needed
    let mut edits = Vec::new();

    // Edit 1: Replace narration with empty string
    let narration_start = narration_info.narration_node.start_position();
    let narration_end = narration_info.narration_node.end_position();

    let narration_range = lsp_types::Range {
        start: lsp_types::Position {
            line: narration_start.row as u32,
            character: narration_start.column as u32,
        },
        end: lsp_types::Position {
            line: narration_end.row as u32,
            character: narration_end.column as u32,
        },
    };

    edits.push(lsp_types::TextEdit {
        range: narration_range,
        new_text: "\"\"".to_string(),
    });

    // Edit 2: Insert metadata line after transaction line
    // For multiline narrations, we need to insert after the narration ends
    let narration_end_line = narration_info.narration_node.end_position().row;
    let narration_end_line_content = content.line(narration_end_line);
    let line_end_char = narration_end_line_content.len_chars();

    // Detect indentation from first posting or use default
    let indent = detect_indentation(&narration_info.transaction_node, content);

    // Escape any quotes in the narration text
    // Preserve newlines - beancount supports multiline strings
    let escaped_text = narration_info.narration_text.replace('"', "\\\"");

    let insertion_point = lsp_types::Position {
        line: narration_end_line as u32,
        character: line_end_char as u32,
    };

    edits.push(lsp_types::TextEdit {
        range: lsp_types::Range {
            start: insertion_point,
            end: insertion_point,
        },
        new_text: format!("\n{}source_desc: \"{}\"", indent, escaped_text),
    });

    // Sort edits from back to front to preserve positions
    edits.sort_by_key(|edit| edit.range.start);
    edits.reverse();

    // Create the workspace edit
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    Some(lsp_types::CodeAction {
        title: "Move narration to source_desc".to_string(),
        kind: Some(lsp_types::CodeActionKind::REFACTOR_REWRITE),
        diagnostics: None,
        edit: Some(lsp_types::WorkspaceEdit::new(changes)),
        command: None,
        is_preferred: Some(true),
        disabled: None,
        data: None,
    })
}

/// Detect indentation for metadata from existing postings or use default
fn detect_indentation(transaction_node: &tree_sitter::Node, content: &ropey::Rope) -> String {
    let mut cursor = transaction_node.walk();

    // Look for first posting child
    for child in transaction_node.children(&mut cursor) {
        if child.kind() == "posting" {
            let posting_line = child.start_position().row;
            let line_content = content.line(posting_line);

            // Count leading whitespace
            let indent_count = line_content
                .chars()
                .take_while(|c| c.is_whitespace())
                .count();

            return " ".repeat(indent_count);
        }
    }

    // Default to 2 spaces if no posting found
    "  ".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn setup_test(content: &str) -> (tree_sitter::Tree, ropey::Rope) {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_beancount::language())
            .unwrap();
        let rope = ropey::Rope::from_str(content);
        let tree = parser.parse(content, None).unwrap();
        (tree, rope)
    }

    #[test]
    fn test_find_narration_with_payee() {
        let content = r#"2022-07-21 * "REWE Essen Limbeck" "Kartenzahlung girocard"
  Assets:Checking  -3.98 EUR
"#;
        let (tree, rope) = setup_test(content);

        // Cursor on narration (second string)
        let position = lsp_types::Position {
            line: 0,
            character: 42, // Inside "Kartenzahlung"
        };

        let result = find_narration_at_cursor(&tree, &rope, position);
        assert!(result.is_some());

        let info = result.unwrap();
        assert_eq!(info.narration_text, "Kartenzahlung girocard");
    }

    #[test]
    fn test_find_narration_without_payee() {
        let content = r#"2022-07-21 * "Kartenzahlung girocard"
  Assets:Checking  -3.98 EUR
"#;
        let (tree, rope) = setup_test(content);

        // Cursor on narration (only string)
        let position = lsp_types::Position {
            line: 0,
            character: 20,
        };

        let result = find_narration_at_cursor(&tree, &rope, position);
        assert!(result.is_some());

        let info = result.unwrap();
        assert_eq!(info.narration_text, "Kartenzahlung girocard");
    }

    #[test]
    fn test_cursor_not_on_narration() {
        let content = r#"2022-07-21 * "REWE" "Test"
  Assets:Checking  -3.98 EUR
"#;
        let (tree, rope) = setup_test(content);

        // Cursor on date
        let position = lsp_types::Position {
            line: 0,
            character: 5,
        };

        let result = find_narration_at_cursor(&tree, &rope, position);
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_narration() {
        let content = r#"2022-07-21 * "REWE" ""
  Assets:Checking  -3.98 EUR
"#;
        let (tree, rope) = setup_test(content);

        let position = lsp_types::Position {
            line: 0,
            character: 25,
        };

        let result = find_narration_at_cursor(&tree, &rope, position);
        assert!(result.is_none()); // Should be None for empty narration
    }

    #[test]
    fn test_has_source_desc_metadata() {
        let content = r#"2022-07-21 * "REWE" "Test"
  source_desc: "Already exists"
  Assets:Checking  -3.98 EUR
"#;
        let (tree, rope) = setup_test(content);

        let root = tree.root_node();
        let transaction = root
            .named_descendant_for_point_range(
                tree_sitter::Point::new(0, 0),
                tree_sitter::Point::new(0, 0),
            )
            .and_then(|node| find_parent_transaction(&node))
            .unwrap();

        assert!(has_source_desc_metadata(&transaction, &rope));
    }

    #[test]
    fn test_detect_indentation_two_spaces() {
        let content = r#"2022-07-21 * "Test" "Test"
  Assets:Checking  -3.98 EUR
"#;
        let (tree, rope) = setup_test(content);

        let root = tree.root_node();
        let transaction = root
            .named_descendant_for_point_range(
                tree_sitter::Point::new(0, 0),
                tree_sitter::Point::new(0, 0),
            )
            .and_then(|node| find_parent_transaction(&node))
            .unwrap();

        let indent = detect_indentation(&transaction, &rope);
        assert_eq!(indent, "  ");
    }

    #[test]
    fn test_detect_indentation_four_spaces() {
        let content = r#"2022-07-21 * "Test" "Test"
    Assets:Checking  -3.98 EUR
"#;
        let (tree, rope) = setup_test(content);

        let root = tree.root_node();
        let transaction = root
            .named_descendant_for_point_range(
                tree_sitter::Point::new(0, 0),
                tree_sitter::Point::new(0, 0),
            )
            .and_then(|node| find_parent_transaction(&node))
            .unwrap();

        let indent = detect_indentation(&transaction, &rope);
        assert_eq!(indent, "    ");
    }

    #[test]
    fn test_detect_indentation_no_posting() {
        let content = r#"2022-07-21 * "Test" "Test"
"#;
        let (tree, rope) = setup_test(content);

        let root = tree.root_node();
        let transaction = root
            .named_descendant_for_point_range(
                tree_sitter::Point::new(0, 0),
                tree_sitter::Point::new(0, 0),
            )
            .and_then(|node| find_parent_transaction(&node))
            .unwrap();

        let indent = detect_indentation(&transaction, &rope);
        assert_eq!(indent, "  "); // Default to 2 spaces
    }

    #[test]
    fn test_multiline_narration() {
        // Test that multiline narrations are detected correctly
        let content = "2022-07-21 * \"REWE\" \"Line 1\nLine 2\nLine 3\"
  Assets:Checking  -3.98 EUR
";
        let (tree, rope) = setup_test(content);

        // Cursor on the multiline narration
        let position = lsp_types::Position {
            line: 0,
            character: 25, // Inside the narration
        };

        let result = find_narration_at_cursor(&tree, &rope, position);
        assert!(result.is_some());

        let info = result.unwrap();
        // The raw narration text should preserve the newlines initially
        assert_eq!(info.narration_text, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_multiline_narration_metadata() {
        // Test that multiline narrations preserve newlines in metadata
        let content = "2022-07-21 * \"Test\" \"Line 1\nLine 2\nLine 3\"
  Assets:Checking  -3.98 EUR
";
        let (tree, rope) = setup_test(content);

        let position = lsp_types::Position {
            line: 0,
            character: 25,
        };

        let narration_info = find_narration_at_cursor(&tree, &rope, position).unwrap();
        let uri = lsp_types::Uri::from_str("file:///tmp/test.beancount").unwrap();

        let action = build_move_to_metadata_action(&rope, &narration_info, &uri);
        assert!(action.is_some());

        let action = action.unwrap();
        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();

        // Should have exactly 2 edits
        assert_eq!(edits.len(), 2);

        // Find the metadata insertion edit (the one that's an insertion, not a replacement)
        let metadata_edit = edits.iter().find(|e| e.range.start == e.range.end).unwrap();

        // Newlines should be preserved - beancount supports multiline strings
        assert_eq!(
            metadata_edit.new_text,
            "\n  source_desc: \"Line 1\nLine 2\nLine 3\""
        );

        // Find the narration replacement edit
        let narration_edit = edits.iter().find(|e| e.range.start != e.range.end).unwrap();
        assert_eq!(narration_edit.new_text, "\"\"");
    }

    #[test]
    fn test_multiline_narration_insertion_point() {
        // Test that insertion point is after narration ends, not after first line
        let content = r#"2021-09-16 * "PayPal" "Basislastschrift
PP.8571.PP"
  Assets:Checking  -6.49 EUR
"#;
        let (tree, rope) = setup_test(content);

        println!("Content:\n{}", content);
        println!("Tree:\n{}", tree.root_node().to_sexp());

        // Cursor on the narration (on first line)
        let position = lsp_types::Position {
            line: 0,
            character: 30, // Inside "Basislastschrift"
        };

        let narration_info = find_narration_at_cursor(&tree, &rope, position).unwrap();
        println!("Narration text: {:?}", narration_info.narration_text);
        println!(
            "Narration node start: {:?}",
            narration_info.narration_node.start_position()
        );
        println!(
            "Narration node end: {:?}",
            narration_info.narration_node.end_position()
        );

        let uri = lsp_types::Uri::from_str("file:///tmp/test.beancount").unwrap();
        let action = build_move_to_metadata_action(&rope, &narration_info, &uri).unwrap();

        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();

        // Debug all edits
        for (i, edit) in edits.iter().enumerate() {
            println!(
                "Edit {}: range={:?}, text={:?}",
                i, edit.range, edit.new_text
            );
        }

        // Find the metadata insertion edit
        let metadata_edit = edits.iter().find(|e| e.range.start == e.range.end).unwrap();
        println!("Metadata edit text: {:?}", metadata_edit.new_text);

        // Find the narration replacement edit
        let narration_edit = edits.iter().find(|e| e.range.start != e.range.end).unwrap();
        println!("Narration edit range: {:?}", narration_edit.range);
        println!("Narration edit text: {:?}", narration_edit.new_text);

        // Metadata should preserve the multiline narration
        assert_eq!(
            metadata_edit.new_text,
            "\n  source_desc: \"Basislastschrift\nPP.8571.PP\""
        );

        // Check narration replacement range - should span from line 0 col 22 to line 1 col 11
        assert_eq!(narration_edit.range.start.line, 0);
        assert_eq!(narration_edit.range.start.character, 22);
        assert_eq!(narration_edit.range.end.line, 1);
        assert_eq!(narration_edit.range.end.character, 11);
        assert_eq!(narration_edit.new_text, "\"\"");

        // Check metadata insertion point - should be AFTER the narration ends (line 1, after col 11)
        assert_eq!(metadata_edit.range.start.line, 1);
        // The line length includes the closing quote and newline
        assert!(metadata_edit.range.start.character >= 11);
    }
}
