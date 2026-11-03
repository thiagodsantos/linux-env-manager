use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) type Variables = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    Bash,
    Zsh,
}

impl Shell {
    pub(crate) fn startup_file(self) -> &'static str {
        match self {
            Self::Bash => "~/.bashrc",
            Self::Zsh => "~/.zshrc",
        }
    }

    pub(crate) fn filename(self) -> &'static str {
        match self {
            Self::Bash => ".bashrc",
            Self::Zsh => ".zshrc",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellIntegration {
    pub shell: Shell,
    pub enabled: bool,
    pub startup_file: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVariableInput {
    pub key: String,
    pub value: String,
    pub original_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InvalidImportLine {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub token: String,
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub invalid_lines: Vec<InvalidImportLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub added: Vec<String>,
    pub updated: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct StoreFile {
    pub(crate) version: u32,
    pub(crate) variables: Variables,
}

#[derive(Default)]
pub(crate) struct ParsedEnv {
    pub(crate) variables: Variables,
    pub(crate) invalid_lines: Vec<InvalidImportLine>,
}

#[derive(Clone)]
pub(crate) struct PendingImport {
    pub(crate) variables: Variables,
    pub(crate) invalid_lines: Vec<InvalidImportLine>,
}
