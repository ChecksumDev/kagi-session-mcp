use async_trait::async_trait;
use reqwest::Client;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, COOKIE, USER_AGENT};
use scraper::{Html, Selector};

use crate::domain::{
    DomainError, DomainResult, FetchRequest, FetchResponse, Session, SessionAuth, UrlFetcher,
};

pub struct ReqwestUrlFetcher {
    client: Client,
}

impl ReqwestUrlFetcher {
    pub fn new() -> DomainResult<Self> {
        let client = Client::builder()
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| DomainError::NetworkError(format!("build http client: {e}")))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl UrlFetcher for ReqwestUrlFetcher {
    async fn fetch(
        &self,
        session: &Session,
        request: &FetchRequest,
    ) -> DomainResult<FetchResponse> {
        let url = url::Url::parse(&request.url)
            .map_err(|e| DomainError::InvalidQuery(format!("invalid url: {e}")))?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(DomainError::InvalidQuery(
                "only http/https urls are allowed".into(),
            ));
        }

        let host = url.host_str().unwrap_or("");
        let is_kagi = host.ends_with("kagi.com") || host.ends_with("kagicdn.com");

        let mut req = self
            .client
            .get(url.clone())
            .header(USER_AGENT, &session.user_agent)
            .header(
                ACCEPT,
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9");

        // Only attach the kagi cookie when fetching kagi.com to avoid leaking it.
        if is_kagi && let SessionAuth::Cookie { name, value } = &session.auth {
            req = req.header(COOKIE, format!("{name}={value}"));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;

        let status = resp.status().as_u16();
        let final_url = resp.url().to_string();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string);
        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;
        let body = String::from_utf8_lossy(&body_bytes).to_string();

        let max_chars = request.max_chars.unwrap_or(50_000);

        let (title, processed) = if request.raw {
            (None, body)
        } else if content_type
            .as_deref()
            .is_none_or(|c| c.contains("text/html"))
        {
            extract_readable(&body)
        } else {
            (None, body)
        };

        let truncated = processed.chars().count() > max_chars;
        let body = if truncated {
            let mut s: String = processed
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect();
            s.push('…');
            s
        } else {
            processed
        };

        Ok(FetchResponse {
            url: request.url.clone(),
            final_url,
            status,
            content_type,
            title,
            body,
            truncated,
        })
    }
}

static TITLE_SEL: std::sync::LazyLock<Selector> =
    std::sync::LazyLock::new(|| Selector::parse("title").unwrap());
static MAIN_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse("main, article, [role=main], #main, #content, .content, body").unwrap()
});

// `if let / else` reads cleaner here than a multi-line map_or_else closure.
#[allow(clippy::option_if_let_else)]
fn extract_readable(html: &str) -> (Option<String>, String) {
    let doc = Html::parse_document(html);
    let title = doc
        .select(&TITLE_SEL)
        .next()
        .map(|el| collapse_ws(&el.text().collect::<String>()))
        .filter(|s| !s.is_empty());

    let text = if let Some(el) = doc.select(&MAIN_SEL).next() {
        render_text(&el)
    } else {
        collapse_ws(&doc.root_element().text().collect::<String>())
    };
    (title, text)
}

fn render_text(el: &scraper::ElementRef<'_>) -> String {
    let mut out = String::new();
    walk(el, &mut out);
    collapse_ws(&out)
}

fn walk(el: &scraper::ElementRef<'_>, out: &mut String) {
    for child in el.children() {
        if let Some(child_el) = scraper::ElementRef::wrap(child) {
            let name = child_el.value().name();
            if matches!(
                name,
                "script" | "style" | "noscript" | "svg" | "iframe" | "link" | "meta"
            ) {
                continue;
            }
            walk(&child_el, out);
        } else if let scraper::Node::Text(t) = child.value() {
            out.push_str(t);
            out.push(' ');
        }
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}
