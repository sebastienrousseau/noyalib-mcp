// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! The prompts: `prompts/list` and `prompts/get`.
//!
//! A prompt is a pure text template. It runs no tool and touches no
//! file; it teaches the host model the recommended `noyalib_get` /
//! `noyalib_set` workflow. [`format_and_lint_yaml`] builds the text;
//! the `#[prompt_router]` block is what the SDK serves.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{GetPromptResult, PromptMessage, Role};
use rmcp::{prompt, prompt_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::YamlServer;

/// The names of the prompts, in the order they are listed.
pub const PROMPT_NAMES: [&str; 1] = ["format_and_lint_yaml"];

/// Arguments of `format_and_lint_yaml`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct FormatAndLintArgs {
    /// Path to the YAML file to review. Optional; omit for a general
    /// workflow.
    #[schemars(example = &"config.yaml")]
    #[serde(default)]
    pub file: Option<String>,
}

/// The `format_and_lint_yaml` prompt text, naming `file` when one is
/// given.
#[must_use]
pub fn format_and_lint_yaml(file: Option<&str>) -> String {
    let target = match file {
        Some(f) if !f.is_empty() => format!("`{f}`"),
        _ => "the YAML file".to_owned(),
    };
    format!(
        "Help me format and lint {target} without losing comments or \
         formatting. First call noyalib_get on the paths you want to inspect \
         to read the current values exactly as written. Propose a minimal set \
         of fixes (indentation, key ordering, quoting, obviously wrong \
         values). For each fix, call noyalib_set with the dotted/indexed path \
         and the replacement YAML fragment: it rewrites only the touched span, \
         so every comment, blank line and sibling entry is preserved \
         byte-for-byte. Re-read with noyalib_get to confirm each change."
    )
}

#[prompt_router(vis = "pub(crate)")]
#[allow(
    clippy::unused_self,
    missing_docs,
    reason = "the SDK's prompt router calls prompts as methods and \
              generates an undocumented descriptor function"
)]
impl YamlServer {
    #[prompt(
        name = "format_and_lint_yaml",
        title = "Format and lint a YAML file (lossless)",
        description = "Guided workflow for inspecting and losslessly \
                       fixing a YAML file with noyalib_get and noyalib_set, preserving \
                       comments and formatting."
    )]
    fn format_and_lint_yaml_prompt(
        &self,
        Parameters(args): Parameters<FormatAndLintArgs>,
    ) -> GetPromptResult {
        GetPromptResult::new(vec![PromptMessage::new_text(
            Role::User,
            format_and_lint_yaml(args.file.as_deref()),
        )])
        .with_description("Guided lossless YAML format-and-lint workflow.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_is_listed_with_its_optional_argument() {
        let prompts = YamlServer::prompt_router().list_all();
        let names: Vec<&str> = prompts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, PROMPT_NAMES);
        let p = &prompts[0];
        assert!(p.title.is_some());
        assert!(p.description.is_some());
        let args = p.arguments.as_ref().expect("arguments");
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "file");
        assert_eq!(args[0].required, Some(false));
        assert!(
            args[0]
                .description
                .as_deref()
                .is_some_and(|d| d.contains("Optional")),
            "{args:?}"
        );
    }

    #[test]
    fn without_a_file_the_text_is_generic() {
        let text = format_and_lint_yaml(None);
        assert!(text.contains("the YAML file"));
        assert!(text.contains("noyalib_get"));
        assert!(text.contains("noyalib_set"));
        assert_eq!(format_and_lint_yaml(Some("")), text);
    }

    #[test]
    fn with_a_file_the_text_names_it() {
        let text = format_and_lint_yaml(Some("config.yml"));
        assert!(text.contains("`config.yml`"));
    }

    #[test]
    fn the_prompt_renders_a_user_message() {
        let result = YamlServer::new().format_and_lint_yaml_prompt(Parameters(FormatAndLintArgs {
            file: Some("deploy.yaml".into()),
        }));
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::User);
        let text = result.messages[0]
            .content
            .as_text()
            .map(|t| t.text.as_str())
            .expect("text");
        assert!(text.contains("`deploy.yaml`"));
        assert!(result.description.is_some());
    }
}
