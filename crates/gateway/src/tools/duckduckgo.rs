use std::sync::Arc;

use anyhow::{Context, Result, bail};
use scraper::{Html, Selector};
use serde::Serialize;
use url::Url;

use super::net::SafeFetcher;

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[async_trait::async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SearchResult>>;
}

/// Unofficial DuckDuckGo HTML endpoint — no API key, but the markup can
/// change or the endpoint can serve a challenge page; errors flow back to
/// the model as tool output.
pub struct DuckDuckGo {
    fetcher: Arc<SafeFetcher>,
}

impl DuckDuckGo {
    pub fn new(fetcher: Arc<SafeFetcher>) -> Self {
        Self { fetcher }
    }
}

#[async_trait::async_trait]
impl SearchProvider for DuckDuckGo {
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        let url = Url::parse_with_params("https://html.duckduckgo.com/html/", [("q", query)])
            .context("failed to build search URL")?;
        let page = self.fetcher.fetch(url.as_str()).await?;
        let mut results = parse_results(&page.body);
        if results.is_empty() {
            bail!(
                "DuckDuckGo returned no results (the query may have no matches, or the endpoint is rate-limiting)"
            );
        }
        results.truncate(max_results);
        Ok(results)
    }
}

pub fn parse_results(html: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let result_selector = Selector::parse("div.result").unwrap();
    let title_selector = Selector::parse("a.result__a").unwrap();
    let snippet_selector = Selector::parse(".result__snippet").unwrap();
    let mut results = Vec::new();
    for element in document.select(&result_selector) {
        if element.value().classes().any(|class| class == "result--ad") {
            continue;
        }
        let Some(link) = element.select(&title_selector).next() else {
            continue;
        };
        let title = collapse(&link.text().collect::<String>());
        let Some(url) = link.value().attr("href").and_then(decode_href) else {
            continue;
        };
        if title.is_empty() {
            continue;
        }
        let snippet = element
            .select(&snippet_selector)
            .next()
            .map(|node| collapse(&node.text().collect::<String>()))
            .unwrap_or_default();
        results.push(SearchResult {
            title,
            url,
            snippet,
        });
    }
    results
}

/// Result hrefs are usually redirect links like
/// `//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2F&rut=…`.
fn decode_href(href: &str) -> Option<String> {
    let absolute = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href.to_owned()
    };
    let url = Url::parse(&absolute).ok()?;
    if url.path().trim_end_matches('/') == "/l" {
        url.query_pairs()
            .find(|(key, _)| key == "uddg")
            .map(|(_, value)| value.into_owned())
    } else {
        Some(absolute)
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
    <html><body>
      <div class="serp__results">
        <div class="result results_links results_links_deep web-result result--ad">
          <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fads.example.com%2F&rut=abc">Sponsored thing</a>
          <a class="result__snippet">Buy now.</a>
        </div>
        <div class="result results_links results_links_deep web-result">
          <h2 class="result__title">
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fdocs.rs%2Faxum%2F&rut=def">Axum —
              web framework</a>
          </h2>
          <a class="result__snippet">Axum is a <b>web</b> application framework.</a>
        </div>
        <div class="result results_links results_links_deep web-result">
          <a class="result__a" href="https://example.com/direct">Direct link result</a>
        </div>
        <div class="result">
          <span>malformed, no link</span>
        </div>
      </div>
    </body></html>
    "#;

    #[test]
    fn parses_results_and_decodes_redirect_links() {
        let results = parse_results(FIXTURE);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Axum — web framework");
        assert_eq!(results[0].url, "https://docs.rs/axum/");
        assert_eq!(results[0].snippet, "Axum is a web application framework.");
        assert_eq!(results[1].url, "https://example.com/direct");
        assert_eq!(results[1].snippet, "");
    }

    #[test]
    fn returns_empty_on_unexpected_markup() {
        assert!(parse_results("<html><body>Anomaly detected</body></html>").is_empty());
    }
}
