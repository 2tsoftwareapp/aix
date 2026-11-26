//! Vendor catalog — JSON-driven provider definitions.
//!
//! The embedded default ships with the app; a user-written
//! ~/.config/aix/vendors.json takes precedence so vendors can be added,
//! removed, or re-pointed (e.g. a corporate proxy base_url) without
//! rebuilding. The file is the single source of truth for both the
//! backend (execution, model listing) and the settings UI.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorConfig {
    pub id: String,
    pub display: String,
    /// "openai_compat" covers every vendor exposing the chat.completions
    /// schema (OpenAI, DeepSeek, Mistral, xAI, Google, …); "anthropic" is
    /// the one native non-compatible API.
    pub kind: String,
    /// URL prefix the `/chat/completions` and `/models` paths hang off —
    /// includes the version segment, which is not always `/v1`
    /// (Google uses /v1beta/openai, DashScope /compatible-mode/v1).
    pub base_url: String,
    /// Optional env var overriding base_url (e.g. DASHSCOPE_BASE_URL for
    /// mainland-China accounts).
    #[serde(default)]
    pub base_url_env: Option<String>,
    /// Environment variable checked as a fallback key source.
    pub env: String,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default)]
    pub source: String,
    /// Extra static headers (OpenRouter wants HTTP-Referer/X-Title).
    #[serde(default)]
    pub extra_headers: BTreeMap<String, String>,
    /// Keep only models whose id starts with one of these prefixes when
    /// listing; empty means keep everything.
    #[serde(default)]
    pub model_prefixes: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VendorFile {
    vendors: Vec<VendorConfig>,
}

const EMBEDDED: &str = include_str!("../config/vendors.json");

fn user_override_path() -> std::path::PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            std::path::PathBuf::from(home).join(".config")
        });
    base.join("aix").join("vendors.json")
}

fn parse(json: &str) -> Vec<VendorConfig> {
    serde_json::from_str::<VendorFile>(json)
        .map(|f| f.vendors)
        .unwrap_or_default()
}

fn load() -> Vec<VendorConfig> {
    if let Ok(body) = std::fs::read_to_string(user_override_path()) {
        let vendors = parse(&body);
        if !vendors.is_empty() {
            return vendors;
        }
    }
    parse(EMBEDDED)
}

static CATALOG: OnceLock<Vec<VendorConfig>> = OnceLock::new();

/// All vendor configs — user override if present and valid, else embedded.
pub fn vendors() -> &'static [VendorConfig] {
    CATALOG.get_or_init(load)
}

pub fn vendor(id: &str) -> Option<&'static VendorConfig> {
    vendors().iter().find(|v| v.id == id)
}

impl VendorConfig {
    /// Effective base URL — env override wins when set.
    pub fn resolved_base_url(&self) -> String {
        if let Some(var) = &self.base_url_env {
            if let Ok(v) = std::env::var(var) {
                if !v.is_empty() {
                    return v;
                }
            }
        }
        self.base_url.clone()
    }

    pub fn matches_model(&self, model_id: &str) -> bool {
        self.model_prefixes.is_empty()
            || self.model_prefixes.iter().any(|p| model_id.starts_with(p.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_parses_and_has_core_vendors() {
        let ids: Vec<&str> = vendors().iter().map(|v| v.id.as_str()).collect();
        for core in ["openrouter", "anthropic", "openai", "deepseek"] {
            assert!(ids.contains(&core), "missing vendor {core}");
        }
    }

    #[test]
    fn every_openai_compat_base_url_has_no_trailing_slash() {
        for v in vendors() {
            assert!(!v.base_url.ends_with('/'), "{} trailing slash", v.id);
        }
    }

    #[test]
    fn model_prefix_filter() {
        let openai = vendor("openai").unwrap();
        assert!(openai.matches_model("gpt-5.2"));
        assert!(openai.matches_model("o4-mini"));
        assert!(!openai.matches_model("text-embedding-3"));
        let openrouter = vendor("openrouter").unwrap();
        assert!(openrouter.matches_model("anything/at-all"));
    }

    #[test]
    fn user_override_file_shape_accepted() {
        let json = r#"{"version":1,"vendors":[{"id":"custom","display":"Custom","kind":"openai_compat","base_url":"https://example.com/v1","env":"CUSTOM_KEY"}]}"#;
        let parsed = parse(json);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "custom");
        assert!(parsed[0].model_prefixes.is_empty());
        assert!(parsed[0].extra_headers.is_empty());
    }
}
