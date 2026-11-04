use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) type Variables = BTreeMap<String, String>;
pub(crate) type Profiles = BTreeMap<String, ProfileData>;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub key: String,
    pub value: String,
    pub inherited: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVariableInput {
    pub profile_name: String,
    pub key: String,
    pub value: String,
    pub original_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileInput {
    pub name: String,
    pub description: String,
    pub color: String,
    pub import_from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub color: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileData {
    pub(crate) description: String,
    pub(crate) color: String,
    pub(crate) variables: Variables,
}

impl ProfileData {
    pub(crate) fn general() -> Self {
        Self {
            description: "Variables shared by all profiles".to_string(),
            color: "#05668D".to_string(),
            variables: Variables::new(),
        }
    }

    pub(crate) fn public(&self, name: &str, active: bool) -> Profile {
        Profile {
            name: name.to_string(),
            description: self.description.clone(),
            color: self.color.clone(),
            active,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoreFile {
    pub(crate) version: u32,
    pub(crate) profiles: Profiles,
    #[serde(default = "default_active_profile")]
    pub(crate) active_profile: String,
}

fn default_active_profile() -> String {
    "General".to_string()
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

#[derive(Default)]
pub(crate) struct ParsedEnv {
    pub(crate) variables: Variables,
    pub(crate) invalid_lines: Vec<InvalidImportLine>,
}

#[derive(Clone)]
pub(crate) struct PendingImport {
    pub(crate) profile_name: String,
    pub(crate) variables: Variables,
    pub(crate) invalid_lines: Vec<InvalidImportLine>,
}
