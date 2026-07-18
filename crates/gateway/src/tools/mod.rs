pub mod duckduckgo;
pub mod fetch_page;
pub mod net;
pub mod web_search;

use std::sync::Arc;

use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::ToolConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub title: String,
    pub url: String,
}

pub struct ToolOutcome {
    pub ok: bool,
    /// Text handed back to the model as the tool message (errors included,
    /// so the model can react to them).
    pub model_content: String,
    /// Structured payload persisted and sent to the UI.
    pub result: Value,
    pub sources: Vec<Source>,
}

impl ToolOutcome {
    pub fn error(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            ok: false,
            result: json!({ "error": &message }),
            model_content: message,
            sources: Vec::new(),
        }
    }
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    /// JSON Schema for the tool arguments.
    fn parameters(&self) -> Value;
    async fn execute(&self, arguments: Value) -> ToolOutcome;
}

pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn standard(config: &ToolConfig) -> Self {
        let fetcher = Arc::new(net::SafeFetcher::new(
            config.timeout,
            config.fetch_max_bytes,
        ));
        Self::new(vec![
            Arc::new(web_search::WebSearchTool::new(
                Arc::new(duckduckgo::DuckDuckGo::new(fetcher.clone())),
                config.search_max_results,
            )),
            Arc::new(fetch_page::FetchPageTool::new(
                fetcher,
                config.page_max_chars,
            )),
        ])
    }

    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        Self { tools }
    }

    pub fn infos(&self) -> Vec<ToolInfo> {
        self.tools
            .iter()
            .map(|tool| ToolInfo {
                tool_type: ToolType::Function,
                function: ToolFunctionInfo {
                    name: tool.name().to_owned(),
                    description: tool.description().to_owned(),
                    parameters: schema_from(tool.parameters()),
                },
            })
            .collect()
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|tool| tool.name() == name).cloned()
    }
}

fn schema_from(value: Value) -> schemars::Schema {
    match value {
        Value::Object(map) => schemars::Schema::from(map),
        _ => schemars::Schema::from(serde_json::Map::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_serializes_to_ollama_tools_json() {
        let registry = ToolRegistry::standard(&ToolConfig::default());
        let infos = serde_json::to_value(registry.infos()).unwrap();
        assert_eq!(infos[0]["type"], "Function");
        assert_eq!(infos[0]["function"]["name"], "web_search");
        assert_eq!(
            infos[0]["function"]["parameters"]["required"],
            json!(["query"])
        );
        assert_eq!(infos[1]["function"]["name"], "fetch_page");
        assert_eq!(
            infos[1]["function"]["parameters"]["properties"]["url"]["type"],
            "string"
        );
    }

    #[test]
    fn registry_lookup_by_name() {
        let registry = ToolRegistry::standard(&ToolConfig::default());
        assert!(registry.get("web_search").is_some());
        assert!(registry.get("fetch_page").is_some());
        assert!(registry.get("unknown").is_none());
    }
}
