use crate::document::Document;
use crate::server::LspServerStateSnapshot;
use crate::treesitter_utils::text_for_tree_sitter_node;
use crate::utils::{self, ToFilePath};
use anyhow::Result;
use lsp_types::Location;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::debug;
use tree_sitter::StreamingIterator;
use tree_sitter_beancount::tree_sitter;

/// Provider function for `textDocument/definition`.
pub(crate) fn definition(
    snapshot: LspServerStateSnapshot,
    params: lsp_types::GotoDefinitionParams,
) -> Result<Option<Vec<lsp_types::LocationLink>>> {
    let uri = params
        .text_document_position_params
        .text_document
        .uri
        .to_file_path()
        .unwrap();
    let line = params.text_document_position_params.position.line;
    let char = params.text_document_position_params.position.character;
    let forest = snapshot.forest;
    let start = tree_sitter::Point {
        row: line as usize,
        column: if char == 0 {
            char as usize
        } else {
            char as usize - 1
        },
    };
    let end = tree_sitter::Point {
        row: line as usize,
        column: char as usize,
    };
    let Some(node) = forest
        .get(&uri)
        .expect("to have tree found")
        .root_node()
        .named_descendant_for_point_range(start, end)
    else {
        return Ok(None);
    };

    let content = snapshot.open_docs.get(&uri).unwrap().content.clone();
    let node_text = text_for_tree_sitter_node(&content, &node);
    let open_docs = snapshot.open_docs;

    match node.grammar_name() {
        "account" => {
            let origin = node.range();

            let locs = find_account_opens(&forest, &open_docs, node_text);
            Ok(Some(
                locs.map(|loc| lsp_types::LocationLink {
                    origin_selection_range: Some(lsp_types::Range {
                        start: lsp_types::Position {
                            line: origin.start_point.row as u32,
                            character: origin.start_point.column as u32,
                        },
                        end: lsp_types::Position {
                            line: origin.end_point.row as u32,
                            character: origin.end_point.column as u32,
                        },
                    }),
                    target_uri: loc.uri,
                    target_range: loc.range,
                    target_selection_range: loc.range,
                })
                .collect(),
            ))
        }
        _ => Ok(None),
    }
}

fn find_account_opens(
    forest: &HashMap<PathBuf, Arc<tree_sitter::Tree>>,
    open_docs: &HashMap<PathBuf, Document>,
    node_text: String,
) -> impl Iterator<Item = lsp_types::Location> {
    forest
        .iter()
        .flat_map(move |(url, tree)| {
            let query = match tree_sitter::Query::new(
                &tree_sitter_beancount::language(),
                "(open (account)@account)",
            ) {
                Ok(q) => q,
                Err(_e) => return vec![],
            };
            let capture_account = query
                .capture_index_for_name("account")
                .expect("account should be captured");
            let text = if open_docs.get(url).is_some() {
                open_docs.get(url).unwrap().text().to_string()
            } else {
                match std::fs::read_to_string(url) {
                    Ok(content) => content,
                    Err(_) => {
                        // If file read fails, return empty results
                        debug!("Failed to read file: {:?}", url);
                        return vec![];
                    }
                }
            };
            let source = text.as_bytes();
            {
                let mut query_cursor = tree_sitter::QueryCursor::new();
                let mut matches = query_cursor.matches(&query, tree.root_node(), source);
                let mut results = Vec::new();
                while let Some(m) = matches.next() {
                    if let Some(node) = m.nodes_for_capture_index(capture_account).next() {
                        let m_text = node.utf8_text(source).expect("");
                        if m_text == node_text {
                            results.push((url.clone(), node));
                        }
                    }
                }
                results
            }
        })
        .map(|(url, node): (PathBuf, tree_sitter::Node)| {
            let range = node.range();
            Location::new(
                utils::path_to_uri(&url),
                lsp_types::Range {
                    start: lsp_types::Position {
                        line: range.start_point.row as u32,
                        character: range.start_point.column as u32,
                    },
                    end: lsp_types::Position {
                        line: range.end_point.row as u32,
                        character: range.end_point.column as u32,
                    },
                },
            )
        })
}
