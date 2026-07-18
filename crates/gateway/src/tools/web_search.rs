use std::sync::Arc;

use serde_json::{Value, json};

use super::{Source, Tool, ToolOutcome, duckduckgo::SearchProvider};

pub struct WebSearchTool {
    provider: Arc<dyn SearchProvider>,
    max_results: usize,
}

impl WebSearchTool {
    pub fn new(provider: Arc<dyn SearchProvider>, max_results: usize) -> Self {
        Self {
            provider,
            max_results,
        }
    }
}

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Search the web. Returns a numbered list of results with title, URL and snippet. Use fetch_page to read a promising result in full."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, arguments: Value) -> ToolOutcome {
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|query| !query.is_empty());
        let Some(query) = query else {
            return ToolOutcome::error("web_search requires a non-empty 'query' string argument");
        };
        match self.provider.search(query, self.max_results).await {
            Ok(results) => {
                let model_content = results
                    .iter()
                    .enumerate()
                    .map(|(index, result)| {
                        format!(
                            "{}. {}\n   {}\n   {}",
                            index + 1,
                            result.title,
                            result.url,
                            result.snippet
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let sources = results
                    .iter()
                    .map(|result| Source {
                        title: result.title.clone(),
                        url: result.url.clone(),
                    })
                    .collect();
                ToolOutcome {
                    ok: true,
                    model_content,
                    result: json!({ "results": results }),
                    sources,
                }
            }
            Err(error) => ToolOutcome::error(format!("web_search failed: {error}")),
        }
    }
}
