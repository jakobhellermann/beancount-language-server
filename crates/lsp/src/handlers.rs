pub mod text_document {
    use crate::providers::completion;
    use crate::providers::formatting;
    use crate::providers::references;
    use crate::providers::semantic_tokens;
    use crate::providers::text_document;
    use crate::server::LspServerState;
    use crate::server::LspServerStateSnapshot;
    use anyhow::Result;

    /// handler for `textDocument/didOpen`.
    pub(crate) fn did_open(
        state: &mut LspServerState,
        params: lsp_types::DidOpenTextDocumentParams,
    ) -> Result<()> {
        tracing::trace!("Document opened: {}", params.text_document.uri.as_str());
        tracing::debug!(
            "Document language: {}, version: {}",
            params.text_document.language_id,
            params.text_document.version
        );
        text_document::did_open(state, params)
    }

    /// handler for `textDocument/didSave`.
    pub(crate) fn did_save(
        state: &mut LspServerState,
        params: lsp_types::DidSaveTextDocumentParams,
    ) -> Result<()> {
        tracing::trace!("Document saved: {}", params.text_document.uri.as_str());
        text_document::did_save(state, params)
    }

    /// handler for `textDocument/didClose`.
    pub(crate) fn did_close(
        state: &mut LspServerState,
        params: lsp_types::DidCloseTextDocumentParams,
    ) -> Result<()> {
        tracing::trace!("Document closed: {}", params.text_document.uri.as_str());
        text_document::did_close(state, params)
    }

    /// handler for `textDocument/didChange`.
    pub(crate) fn did_change(
        state: &mut LspServerState,
        params: lsp_types::DidChangeTextDocumentParams,
    ) -> Result<()> {
        tracing::debug!(
            "Document changed: {}, version: {}",
            params.text_document.uri.as_str(),
            params.text_document.version
        );
        tracing::debug!(
            "Number of content changes: {}",
            params.content_changes.len()
        );
        text_document::did_change(state, params)
    }

    pub(crate) fn completion(
        snapshot: LspServerStateSnapshot,
        params: lsp_types::CompletionParams,
    ) -> anyhow::Result<Option<lsp_types::CompletionResponse>> {
        tracing::debug!(
            "Completion requested for: {} at {}:{}",
            params.text_document_position.text_document.uri.as_str(),
            params.text_document_position.position.line,
            params.text_document_position.position.character
        );

        let trigger_char = match &params.context {
            Some(context) => match &context.trigger_character {
                Some(trigger_character) => {
                    tracing::debug!("Completion triggered by character: '{}'", trigger_character);
                    if trigger_character == "2" {
                        if params.text_document_position.position.character > 1 {
                            None
                        } else {
                            trigger_character.chars().last()
                        }
                    } else {
                        trigger_character.chars().last()
                    }
                }
                None => {
                    tracing::debug!("Completion triggered manually (no trigger character)");
                    None
                }
            },
            None => {
                tracing::debug!("Completion triggered manually (no context)");
                None
            }
        };

        match completion::completion(snapshot, trigger_char, params.text_document_position) {
            Ok(Some(items)) => {
                tracing::trace!("Completion returned {} items", items.len());
                // Return CompletionList instead of Array to signal that server-side
                // filtering is preferred. Setting `is_incomplete: true` tells clients
                // like Zed to re-query on each keystroke rather than filtering internally.
                Ok(Some(lsp_types::CompletionResponse::List(
                    lsp_types::CompletionList {
                        is_incomplete: true,
                        items,
                    },
                )))
            }
            Ok(None) => {
                tracing::debug!("No completion items available");
                Ok(None)
            }
            Err(e) => {
                tracing::error!("Completion failed: {}", e);
                Err(e)
            }
        }
    }

    pub(crate) fn formatting(
        snapshot: LspServerStateSnapshot,
        params: lsp_types::DocumentFormattingParams,
    ) -> Result<Option<Vec<lsp_types::TextEdit>>> {
        tracing::trace!(
            "Formatting requested for: {}",
            params.text_document.uri.as_str()
        );
        tracing::debug!(
            "Formatting options: tab_size={}, insert_spaces={}",
            params.options.tab_size,
            params.options.insert_spaces
        );

        match formatting::formatting(snapshot, params) {
            Ok(Some(edits)) => {
                tracing::trace!("Formatting returned {} text edits", edits.len());
                Ok(Some(edits))
            }
            Ok(None) => {
                tracing::debug!("No formatting changes needed");
                Ok(None)
            }
            Err(e) => {
                tracing::error!("Formatting failed: {}", e);
                Err(e)
            }
        }
    }

    /// handler for `textDocument/willSaveWaitUntil`.
    pub(crate) fn will_save_wait_until(
        snapshot: LspServerStateSnapshot,
        params: lsp_types::WillSaveTextDocumentParams,
    ) -> Result<Option<Vec<lsp_types::TextEdit>>> {
        tracing::trace!(
            "WillSaveWaitUntil requested for: {}",
            params.text_document.uri.as_str()
        );

        // Convert WillSaveTextDocumentParams to DocumentFormattingParams
        let formatting_params = lsp_types::DocumentFormattingParams {
            text_document: params.text_document,
            options: lsp_types::FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                ..Default::default()
            },
            work_done_progress_params: Default::default(),
        };

        match formatting::formatting(snapshot, formatting_params) {
            Ok(Some(edits)) => {
                tracing::trace!("WillSaveWaitUntil returned {} text edits", edits.len());
                Ok(Some(edits))
            }
            Ok(None) => {
                tracing::debug!("No formatting changes needed before save");
                Ok(None)
            }
            Err(e) => {
                tracing::error!("WillSaveWaitUntil formatting failed: {}", e);
                Err(e)
            }
        }
    }

    pub(crate) fn handle_references(
        snapshot: LspServerStateSnapshot,
        params: lsp_types::ReferenceParams,
    ) -> Result<Option<Vec<lsp_types::Location>>> {
        tracing::trace!(
            "References requested for: {} at {}:{}",
            params.text_document_position.text_document.uri.as_str(),
            params.text_document_position.position.line,
            params.text_document_position.position.character
        );

        match references::references(snapshot, params) {
            Ok(Some(locations)) => {
                tracing::trace!("Found {} references", locations.len());
                Ok(Some(locations))
            }
            Ok(None) => {
                tracing::debug!("No references found");
                Ok(None)
            }
            Err(e) => {
                tracing::error!("References lookup failed: {}", e);
                Err(e)
            }
        }
    }

    pub(crate) fn handle_rename(
        snapshot: LspServerStateSnapshot,
        params: lsp_types::RenameParams,
    ) -> Result<Option<lsp_types::WorkspaceEdit>> {
        tracing::trace!(
            "Rename requested for: {} at {}:{} to '{}'",
            params.text_document_position.text_document.uri.as_str(),
            params.text_document_position.position.line,
            params.text_document_position.position.character,
            params.new_name
        );

        match references::rename(snapshot, params) {
            Ok(Some(workspace_edit)) => {
                let change_count = workspace_edit
                    .changes
                    .as_ref()
                    .map(|changes| changes.values().map(|edits| edits.len()).sum::<usize>())
                    .unwrap_or(0);
                tracing::trace!("Rename will make {} text edits", change_count);
                Ok(Some(workspace_edit))
            }
            Ok(None) => {
                tracing::debug!("No rename edits generated");
                Ok(None)
            }
            Err(e) => {
                tracing::error!("Rename failed: {}", e);
                Err(e)
            }
        }
    }

    pub(crate) fn semantic_tokens_full(
        snapshot: LspServerStateSnapshot,
        params: lsp_types::SemanticTokensParams,
    ) -> Result<Option<lsp_types::SemanticTokensResult>> {
        tracing::debug!(
            "Semantic tokens requested for: {}",
            params.text_document.uri.as_str()
        );
        semantic_tokens::semantic_tokens_full(snapshot, params)
    }
}

pub mod configuration {
    use crate::server::LspServerState;
    use anyhow::Result;

    /// handler for `workspace/didChangeConfiguration`.
    pub(crate) fn did_change(
        state: &mut LspServerState,
        params: lsp_types::DidChangeConfigurationParams,
    ) -> Result<()> {
        tracing::info!("Configuration changed: {}", params.settings);

        match state.config.update(params.settings) {
            Ok(()) => tracing::debug!("Configuration updated successfully"),
            Err(e) => {
                tracing::warn!("Failed to update configuration: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::config::Config;
        use std::path::PathBuf;

        fn create_test_state() -> LspServerState {
            let config = Config::new(PathBuf::new());
            let (sender, _receiver) = crossbeam_channel::unbounded();
            LspServerState::new(sender, config)
        }

        #[test]
        fn test_did_change_updates_journal_file() {
            let mut state = create_test_state();
            let params = lsp_types::DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "journal_file": "/path/to/journal.beancount"
                }),
            };

            did_change(&mut state, params).unwrap();

            assert_eq!(
                state.config.journal_root,
                Some(PathBuf::from("/path/to/journal.beancount"))
            );
        }

        #[test]
        fn test_did_change_updates_formatting_options() {
            let mut state = create_test_state();
            let params = lsp_types::DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "formatting": {
                        "prefix_width": 10,
                        "num_width": 12,
                        "currency_column": 80,
                        "account_amount_spacing": 4,
                        "number_currency_spacing": 2
                    }
                }),
            };

            did_change(&mut state, params).unwrap();

            assert_eq!(state.config.formatting.prefix_width, Some(10));
            assert_eq!(state.config.formatting.num_width, Some(12));
            assert_eq!(state.config.formatting.currency_column, Some(80));
            assert_eq!(state.config.formatting.account_amount_spacing, 4);
            assert_eq!(state.config.formatting.number_currency_spacing, 2);
        }

        #[test]
        fn test_did_change_updates_bean_check_method() {
            let mut state = create_test_state();
            let params = lsp_types::DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "bean_check": {
                        "method": "python-embedded"
                    }
                }),
            };

            did_change(&mut state, params).unwrap();

            assert_eq!(
                state.config.bean_check.method,
                crate::checkers::BeancountCheckMethod::PythonEmbedded
            );
        }

        #[test]
        fn test_did_change_handles_empty_settings() {
            let mut state = create_test_state();
            let original_journal = state.config.journal_root.clone();

            let params = lsp_types::DidChangeConfigurationParams {
                settings: serde_json::json!({}),
            };

            did_change(&mut state, params).unwrap();

            // Config should remain unchanged
            assert_eq!(state.config.journal_root, original_journal);
        }

        #[test]
        fn test_did_change_partial_update() {
            let mut state = create_test_state();

            // First update
            let params1 = lsp_types::DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "journal_file": "/path/to/journal.beancount",
                    "formatting": {
                        "prefix_width": 10
                    }
                }),
            };
            did_change(&mut state, params1).unwrap();

            // Second update that only changes formatting
            let params2 = lsp_types::DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "formatting": {
                        "num_width": 15
                    }
                }),
            };
            did_change(&mut state, params2).unwrap();

            // Both updates should be preserved
            assert_eq!(
                state.config.journal_root,
                Some(PathBuf::from("/path/to/journal.beancount"))
            );
            assert_eq!(state.config.formatting.prefix_width, Some(10));
            assert_eq!(state.config.formatting.num_width, Some(15));
        }
    }
}
