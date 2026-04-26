use async_trait::async_trait;
use reqwest::Client;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, COOKIE, USER_AGENT};
use serde::Deserialize;
use url::Url;

use crate::domain::{
    DomainError, DomainResult, FastGptAnswer, KagiClient, KagiVertical, Lens, QuickAnswer,
    SearchQuery, SearchResponse, SearchResult, Session, SessionAuth, Suggestion, VerticalQuery,
    VerticalResponse,
};

use super::serp_parser::{
    parse_fastgpt_answer, parse_lens_settings, parse_related_searches, parse_serp,
    parse_top_content_unique, parse_vertical_serp, parse_wikipedia_panel,
};

const KAGI_BASE: &str = "https://kagi.com";

// /socket/search returns SSE frames whose payloads are JSON arrays of
// {tag, payload} messages. Tags we consume:
//   search, interesting-finds  -> result-row HTML (main results)
//   wikipedia                  -> knowledge panel HTML
//   related_searches           -> follow-up queries
//   top-content-unique         -> total-result-count chunk
//   search.info                -> pagination metadata
//   news, videos, images, podcasts -> per-vertical result HTML
pub struct ReqwestKagiClient {
    client: Client,
}

impl ReqwestKagiClient {
    pub fn new() -> DomainResult<Self> {
        let client = Client::builder()
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .build()
            .map_err(|e| DomainError::NetworkError(format!("build http client: {e}")))?;
        Ok(Self { client })
    }

    pub const fn http(&self) -> &Client {
        &self.client
    }

    fn authed_get(&self, url: Url, session: &Session) -> reqwest::RequestBuilder {
        let mut req = self
            .client
            .get(url)
            .header(USER_AGENT, &session.user_agent)
            .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9");
        if let SessionAuth::Cookie { name, value } = &session.auth {
            req = req.header(COOKIE, format!("{name}={value}"));
        }
        req
    }
}

#[async_trait]
impl KagiClient for ReqwestKagiClient {
    async fn search(&self, session: &Session, query: &SearchQuery) -> DomainResult<SearchResponse> {
        if query.q.trim().is_empty() {
            return Err(DomainError::InvalidQuery("query is empty".into()));
        }
        // Lens param accepts toolbar slot indices 0..7. Out-of-range values
        // are silently ignored by Kagi; surface an actionable error so the
        // caller doesn't think the lens applied.
        if let Some(lens) = query.lens
            && lens > 7
        {
            return Err(DomainError::InvalidQuery(format!(
                "lens slot {lens} is out of range (valid: 0-7); call kagi_list_lenses to see your active lenses in slot order"
            )));
        }

        let url = build_search_url(query, session)?;

        let resp = self
            .authed_get(url, session)
            .header(ACCEPT, "text/event-stream")
            .send()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;

        let status = resp.status();
        let final_url = resp.url().clone();
        if final_url.path().starts_with("/signin") {
            return Err(DomainError::SessionRejected(format!(
                "kagi redirected to {} (HTTP {status}); session is missing or expired",
                final_url.path()
            )));
        }
        if !status.is_success() {
            return Err(DomainError::NetworkError(format!(
                "kagi returned HTTP {status}"
            )));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;

        let parts = collect_stream_parts(&body);

        let mut combined_html = String::new();
        for s in &parts.search {
            combined_html.push_str(s);
            combined_html.push('\n');
        }
        for s in &parts.interesting {
            combined_html.push_str(s);
            combined_html.push('\n');
        }

        let mut results: Vec<SearchResult> = if combined_html.trim().is_empty() {
            Vec::new()
        } else {
            parse_serp(&combined_html)?
        };

        if let Some(limit) = query.limit {
            results.truncate(limit as usize);
        }
        for (i, r) in results.iter_mut().enumerate() {
            r.rank = (i + 1) as u32;
        }

        let quick_answer: Option<QuickAnswer> = parts
            .wikipedia
            .iter()
            .find_map(|html| parse_wikipedia_panel(html));

        let mut related_queries: Vec<String> = Vec::new();
        for html in &parts.related {
            for q in parse_related_searches(html) {
                if !related_queries.iter().any(|r: &String| r == &q) {
                    related_queries.push(q);
                }
            }
        }

        let total_results = parts
            .top_content_unique
            .iter()
            .find_map(|html| parse_top_content_unique(html));
        // Kagi sends -1 for next_batch when this was the last page.
        let next_page = parts.search_info.iter().find_map(|info| {
            let n = info.get("next_batch").and_then(serde_json::Value::as_i64)?;
            if n > 0 { Some(n as u32) } else { None }
        });

        if results.is_empty() && quick_answer.is_none() && parts.is_empty() {
            return Err(DomainError::ParseError(
                "kagi stream contained no recognised payloads (session may be valid but the protocol may have changed)".into(),
            ));
        }

        Ok(SearchResponse {
            results,
            quick_answer,
            related_queries,
            total_results,
            next_page,
        })
    }

    async fn wikipedia(&self, session: &Session, query: &str) -> DomainResult<QuickAnswer> {
        if query.trim().is_empty() {
            return Err(DomainError::InvalidQuery("query is empty".into()));
        }
        let mut url = Url::parse(&format!("{KAGI_BASE}/api/wikipedia"))
            .map_err(|e| DomainError::NetworkError(format!("url parse: {e}")))?;
        url.query_pairs_mut().append_pair("q", query);

        let resp = self
            .authed_get(url, session)
            .header(ACCEPT, "text/html")
            .send()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;
        check_signin(&resp)?;
        if !resp.status().is_success() {
            return Err(DomainError::NetworkError(format!(
                "kagi /api/wikipedia returned HTTP {}",
                resp.status()
            )));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;
        parse_wikipedia_panel(&body).ok_or_else(|| {
            DomainError::ParseError(
                "wikipedia endpoint returned no recognisable knowledge panel".into(),
            )
        })
    }

    async fn suggest(&self, session: &Session, prefix: &str) -> DomainResult<Vec<Suggestion>> {
        if prefix.is_empty() {
            return Ok(Vec::new());
        }
        let mut url = Url::parse(&format!("{KAGI_BASE}/autosuggest"))
            .map_err(|e| DomainError::NetworkError(format!("url parse: {e}")))?;
        url.query_pairs_mut().append_pair("q", prefix);

        let resp = self
            .authed_get(url, session)
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;
        check_signin(&resp)?;
        if !resp.status().is_success() {
            return Err(DomainError::NetworkError(format!(
                "kagi /autosuggest returned HTTP {}",
                resp.status()
            )));
        }
        let raw: Vec<RawSuggest> = resp
            .json()
            .await
            .map_err(|e| DomainError::ParseError(format!("autosuggest json: {e}")))?;
        Ok(raw
            .into_iter()
            .map(|r| Suggestion {
                text: r.t,
                description: r.txt,
                image_url: r.img,
                source: r.k,
            })
            .collect())
    }

    async fn fastgpt(&self, session: &Session, query: &str) -> DomainResult<FastGptAnswer> {
        if query.trim().is_empty() {
            return Err(DomainError::InvalidQuery("query is empty".into()));
        }
        let mut url = Url::parse(&format!("{KAGI_BASE}/stream_fastgpt"))
            .map_err(|e| DomainError::NetworkError(format!("url parse: {e}")))?;
        url.query_pairs_mut().append_pair("query", query);

        let resp = self
            .authed_get(url, session)
            .header(ACCEPT, "text/event-stream")
            .send()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;
        check_signin(&resp)?;
        if !resp.status().is_success() {
            return Err(DomainError::NetworkError(format!(
                "kagi /stream_fastgpt returned HTTP {}",
                resp.status()
            )));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;

        // FastGPT frames are progressive: each one is the full answer so far,
        // terminated by a literal "[DONE]" sentinel. Take the last non-sentinel.
        let chunks = collect_fastgpt_chunks(&body);
        let final_html = chunks
            .iter()
            .rev()
            .find(|c| c.trim() != "[DONE]")
            .cloned()
            .unwrap_or_default();
        if final_html.is_empty() {
            return Err(DomainError::ParseError(
                "fastgpt stream contained no answer frames".into(),
            ));
        }
        Ok(parse_fastgpt_answer(&final_html))
    }

    async fn vertical_search(
        &self,
        session: &Session,
        vertical: KagiVertical,
        query: &VerticalQuery,
    ) -> DomainResult<VerticalResponse> {
        if query.q.trim().is_empty() {
            return Err(DomainError::InvalidQuery("query is empty".into()));
        }
        let mut url = Url::parse(&format!("{KAGI_BASE}{}", vertical.path()))
            .map_err(|e| DomainError::NetworkError(format!("url parse: {e}")))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("q", &query.q);
            if let Some(t) = query.freshness {
                q.append_pair("freshness", t.as_freshness());
            }
            if let Some(o) = query.order {
                q.append_pair("order", &o.to_string());
            }
            if query.dir_desc {
                q.append_pair("dir", "desc");
            }
            if let Some(ai) = &query.ai_filter {
                q.append_pair("ai", ai);
            }
            if let Some(asp) = &query.aspect {
                q.append_pair("aspect", asp);
            }
            if let Some(c) = &query.color {
                q.append_pair("color", c);
            }
            if let Some(d) = &query.duration {
                q.append_pair("duration", d);
            }
        }

        // Verticals are content-negotiated by Accept: without text/event-stream
        // you get the SPA shell with empty result containers.
        let resp = self
            .authed_get(url, session)
            .header(ACCEPT, "text/event-stream")
            .send()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;
        check_signin(&resp)?;
        if !resp.status().is_success() {
            return Err(DomainError::NetworkError(format!(
                "kagi {} returned HTTP {}",
                vertical.path(),
                resp.status()
            )));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;

        let parts = collect_stream_parts(&body);
        let chunks = parts.vertical(vertical);
        if chunks.is_empty() && parts.is_empty() {
            return Err(DomainError::ParseError(format!(
                "kagi {} stream contained no recognised payloads",
                vertical.name()
            )));
        }

        let mut combined = String::new();
        for c in chunks {
            combined.push_str(c);
            combined.push('\n');
        }
        let mut results = parse_vertical_serp(&combined, vertical)?;
        if let Some(limit) = query.limit {
            results.truncate(limit as usize);
        }
        for (i, r) in results.iter_mut().enumerate() {
            r.rank = (i + 1) as u32;
        }
        Ok(VerticalResponse { results })
    }

    async fn list_lenses(&self, session: &Session) -> DomainResult<Vec<Lens>> {
        let url = Url::parse(&format!("{KAGI_BASE}/settings/lenses"))
            .map_err(|e| DomainError::NetworkError(format!("url parse: {e}")))?;
        let resp = self
            .authed_get(url, session)
            .header(ACCEPT, "text/html")
            .send()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;
        check_signin(&resp)?;
        if !resp.status().is_success() {
            return Err(DomainError::NetworkError(format!(
                "kagi /settings/lenses returned HTTP {}",
                resp.status()
            )));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| DomainError::NetworkError(e.to_string()))?;
        Ok(parse_lens_settings(&body))
    }
}

#[derive(Debug, Deserialize)]
struct RawSuggest {
    t: String,
    txt: Option<String>,
    img: Option<String>,
    k: Option<String>,
}

fn check_signin(resp: &reqwest::Response) -> DomainResult<()> {
    if resp.url().path().starts_with("/signin") {
        return Err(DomainError::SessionRejected(format!(
            "kagi redirected to {}; session is missing or expired",
            resp.url().path()
        )));
    }
    Ok(())
}

fn build_search_url(query: &SearchQuery, session: &Session) -> DomainResult<Url> {
    let mut url = Url::parse(&format!("{KAGI_BASE}/socket/search"))
        .map_err(|e| DomainError::NetworkError(format!("url parse: {e}")))?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("q", &query.q);
        // Kagi paginates by &batch=N&piece=M, not &page=N. piece=1 covers the
        // primary results group; higher pieces subdivide specialised sections.
        if let Some(page) = query.page.filter(|p| *p > 1) {
            q.append_pair("batch", &page.to_string());
            q.append_pair("piece", "1");
        }
        if let Some(t) = query.time {
            q.append_pair("date", t.as_kagi());
        }
        if let Some(region) = &query.region {
            q.append_pair("r", region);
        }
        if let Some(safe) = query.safe {
            q.append_pair("safe", safe.as_kagi());
        }
        if let Some(lens) = query.lens {
            // &l= takes a toolbar slot index (0..7), not the global lens id.
            q.append_pair("l", &lens.to_string());
        }
        if let SessionAuth::UrlToken(token) = &session.auth {
            q.append_pair("token", token);
        }
    }
    Ok(url)
}

#[derive(Debug, Default)]
struct StreamParts {
    search: Vec<String>,
    interesting: Vec<String>,
    wikipedia: Vec<String>,
    related: Vec<String>,
    top_content_unique: Vec<String>,
    search_info: Vec<serde_json::Value>,
    news: Vec<String>,
    videos: Vec<String>,
    images: Vec<String>,
    podcasts: Vec<String>,
}

impl StreamParts {
    fn is_empty(&self) -> bool {
        self.search.is_empty()
            && self.interesting.is_empty()
            && self.wikipedia.is_empty()
            && self.related.is_empty()
            && self.top_content_unique.is_empty()
            && self.search_info.is_empty()
            && self.news.is_empty()
            && self.videos.is_empty()
            && self.images.is_empty()
            && self.podcasts.is_empty()
    }

    fn vertical(&self, v: KagiVertical) -> &[String] {
        match v {
            KagiVertical::News => &self.news,
            KagiVertical::Videos => &self.videos,
            KagiVertical::Images => &self.images,
            KagiVertical::Podcasts => &self.podcasts,
        }
    }
}

#[derive(Debug, Deserialize)]
struct StreamMessage {
    tag: String,
    payload: serde_json::Value,
}

fn collect_stream_parts(body: &str) -> StreamParts {
    let mut parts = StreamParts::default();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("data:") else {
            continue;
        };
        let json = rest.trim_start();
        if json.is_empty() {
            continue;
        }
        let Ok(msgs) = serde_json::from_str::<Vec<StreamMessage>>(json) else {
            continue;
        };
        for m in msgs {
            if m.tag == "search.info" {
                parts.search_info.push(m.payload);
                continue;
            }
            // Older builds emit string payloads; current builds wrap them in
            // an object with a `content` field.
            let html = match &m.payload {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Object(_) => m
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                _ => None,
            };
            let Some(html) = html else { continue };
            match m.tag.as_str() {
                "search" => parts.search.push(html),
                "interesting-finds" => parts.interesting.push(html),
                "wikipedia" => parts.wikipedia.push(html),
                "related_searches" => parts.related.push(html),
                "top-content-unique" => parts.top_content_unique.push(html),
                "news" => parts.news.push(html),
                "videos" => parts.videos.push(html),
                "images" => parts.images.push(html),
                "podcasts" => parts.podcasts.push(html),
                _ => {}
            }
        }
    }
    parts
}

fn collect_fastgpt_chunks(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("data:") else {
            continue;
        };
        let json = rest.trim_start();
        if json.is_empty() {
            continue;
        }
        if let Ok(s) = serde_json::from_str::<String>(json) {
            out.push(s);
        }
    }
    out
}
