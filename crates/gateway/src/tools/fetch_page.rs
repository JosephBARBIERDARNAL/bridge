use std::sync::Arc;

use scraper::{Html, Node, Selector};
use serde_json::{Value, json};

use super::{Source, Tool, ToolOutcome, net::SafeFetcher};

pub struct FetchPageTool {
    fetcher: Arc<SafeFetcher>,
    max_chars: usize,
}

impl FetchPageTool {
    pub fn new(fetcher: Arc<SafeFetcher>, max_chars: usize) -> Self {
        Self { fetcher, max_chars }
    }
}

#[async_trait::async_trait]
impl Tool for FetchPageTool {
    fn name(&self) -> &'static str {
        "fetch_page"
    }

    fn description(&self) -> &'static str {
        "Fetch a web page and return its readable text content (truncated). Use it to read a web_search result in full."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The http(s) URL of the page to fetch"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, arguments: Value) -> ToolOutcome {
        let url = arguments
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty());
        let Some(url) = url else {
            return ToolOutcome::error("fetch_page requires a non-empty 'url' string argument");
        };
        let page = match self.fetcher.fetch(url).await {
            Ok(page) => page,
            Err(error) => return ToolOutcome::error(format!("fetch_page failed: {error}")),
        };
        let extracted = if page.content_type.contains("html") {
            extract_readable(&page.body)
        } else {
            ExtractedPage {
                title: String::new(),
                text: page.body,
            }
        };
        let (text, truncated) = truncate_chars(&extracted.text, self.max_chars);
        if text.is_empty() {
            return ToolOutcome::error("fetch_page found no readable text on the page");
        }
        let title = if extracted.title.is_empty() {
            page.final_url.host_str().unwrap_or("page").to_owned()
        } else {
            extracted.title.clone()
        };
        let mut model_content = format!("Content of {} ({}):\n\n{}", page.final_url, title, text);
        if truncated || page.truncated {
            model_content.push_str("\n\n…[truncated]");
        }
        ToolOutcome {
            ok: true,
            model_content,
            result: json!({
                "url": page.final_url.as_str(),
                "title": title,
                "excerpt": text,
                "truncated": truncated || page.truncated,
            }),
            sources: vec![Source {
                title,
                url: page.final_url.to_string(),
            }],
        }
    }
}

pub struct ExtractedPage {
    pub title: String,
    pub text: String,
}

const SKIPPED_ELEMENTS: [&str; 12] = [
    "script", "style", "noscript", "template", "svg", "nav", "header", "footer", "aside", "form",
    "iframe", "select",
];

const BLOCK_ELEMENTS: [&str; 17] = [
    "p",
    "div",
    "li",
    "ul",
    "ol",
    "br",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "tr",
    "table",
    "section",
    "article",
    "blockquote",
];

pub fn extract_readable(html: &str) -> ExtractedPage {
    let document = Html::parse_document(html);
    let title = document
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|node| collapse_inline(&node.text().collect::<String>()))
        .unwrap_or_default();

    let root = ["article", "main", "[role=\"main\"]", "body"]
        .iter()
        .find_map(|selector| document.select(&Selector::parse(selector).unwrap()).next());
    let mut raw = String::new();
    collect_text(root.unwrap_or_else(|| document.root_element()), &mut raw);

    let text = raw
        .lines()
        .map(collapse_inline)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    ExtractedPage { title, text }
}

fn collect_text(element: scraper::ElementRef<'_>, out: &mut String) {
    for child in element.children() {
        match child.value() {
            Node::Text(text) => out.push_str(&text.replace(['\n', '\r'], " ")),
            Node::Element(child_element) => {
                let name = child_element.name();
                if SKIPPED_ELEMENTS.contains(&name) {
                    continue;
                }
                let block = BLOCK_ELEMENTS.contains(&name);
                if block {
                    out.push('\n');
                }
                if let Some(child_ref) = scraper::ElementRef::wrap(child) {
                    collect_text(child_ref, out);
                }
                if block {
                    out.push('\n');
                }
            }
            _ => {}
        }
    }
}

fn collapse_inline(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max_chars: usize) -> (String, bool) {
    match text.char_indices().nth(max_chars) {
        Some((offset, _)) => (text[..offset].trim_end().to_owned(), true),
        None => (text.to_owned(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
    <html>
      <head>
        <title>  My   Article </title>
        <style>body { color: red; }</style>
        <script>console.log("tracking");</script>
      </head>
      <body>
        <nav><a href="/">Home</a><a href="/about">About</a></nav>
        <header>Site header</header>
        <article>
          <h1>My Article</h1>
          <p>First <b>paragraph</b> of
             the article.</p>
          <script>evil()</script>
          <p>Second paragraph.</p>
          <aside>Related links you do not care about</aside>
        </article>
        <footer>Copyright</footer>
      </body>
    </html>
    "#;

    #[test]
    fn extracts_readable_text_from_article() {
        let page = extract_readable(FIXTURE);
        assert_eq!(page.title, "My Article");
        assert_eq!(
            page.text,
            "My Article\nFirst paragraph of the article.\nSecond paragraph."
        );
    }

    #[test]
    fn falls_back_to_body_and_skips_chrome() {
        let page = extract_readable(
            "<html><body><nav>menu</nav><p>Hello <i>world</i></p><footer>foot</footer></body></html>",
        );
        assert_eq!(page.text, "Hello world");
        assert_eq!(page.title, "");
    }

    #[test]
    fn truncates_by_characters() {
        let (text, truncated) = truncate_chars("héllo world", 5);
        assert_eq!(text, "héllo");
        assert!(truncated);
        let (text, truncated) = truncate_chars("short", 10);
        assert_eq!(text, "short");
        assert!(!truncated);
    }
}
