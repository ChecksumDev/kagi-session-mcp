use serde::{Deserialize, Serialize};

use crate::domain::{
    FastGptAnswer, FastGptSource, FetchRequest, FetchResponse, KagiVertical, Lens, QuickAnswer,
    SafeSearch, SearchQuery, SearchResponse, SearchResult, Suggestion, TimeRange, VerticalQuery,
    VerticalResponse, VerticalResult,
};

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SearchInput {
    /// The search query. Supports Kagi operators: site:, filetype:,
    /// -exclude, "exact phrase", before:YYYY-MM-DD, after:YYYY-MM-DD.
    pub query: String,
    /// Maximum number of results (1 to 25). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u8>,
    /// 1-based page number. Each page is one Kagi batch (fresh results,
    /// not a slice of page 1). Defaults to 1.
    #[serde(default)]
    pub page: Option<u32>,
    /// Restrict to recent results: "day", "week", "month", "year".
    #[serde(default)]
    pub time: Option<String>,
    /// Region code (e.g. "us", "de", "jp") to bias geographically.
    #[serde(default)]
    pub region: Option<String>,
    /// Adult content filter: "off", "moderate", "strict".
    #[serde(default)]
    pub safe: Option<String>,
    /// Apply a Kagi Lens by toolbar slot index (0..7). Slot 0 is the
    /// user's first active lens. Call `kagi_list_lenses` to see the order.
    #[serde(default)]
    pub lens: Option<u32>,
}

impl SearchInput {
    pub fn into_domain(self) -> SearchQuery {
        SearchQuery {
            q: self.query,
            limit: Some(self.limit.unwrap_or(10).clamp(1, 25)),
            page: self.page.filter(|p| *p > 0),
            time: self.time.as_deref().and_then(parse_time),
            region: self.region,
            safe: self.safe.as_deref().and_then(parse_safe),
            lens: self.lens.filter(|l| *l > 0),
        }
    }
}

fn parse_time(s: &str) -> Option<TimeRange> {
    match s.to_ascii_lowercase().as_str() {
        "day" | "d" => Some(TimeRange::PastDay),
        "week" | "w" => Some(TimeRange::PastWeek),
        "month" | "m" => Some(TimeRange::PastMonth),
        "year" | "y" => Some(TimeRange::PastYear),
        _ => None,
    }
}

fn parse_safe(s: &str) -> Option<SafeSearch> {
    match s.to_ascii_lowercase().as_str() {
        "off" | "0" => Some(SafeSearch::Off),
        "moderate" | "1" => Some(SafeSearch::Moderate),
        "strict" | "2" => Some(SafeSearch::Strict),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchPayload {
    pub query: String,
    pub count: usize,
    pub results: Vec<MinimalResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_answer: Option<MinimalQuickAnswer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_results: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinimalResult {
    pub n: u32,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

impl From<SearchResult> for MinimalResult {
    fn from(r: SearchResult) -> Self {
        Self {
            n: r.rank,
            title: r.title,
            url: r.url,
            snippet: r.snippet,
            site: r.site,
            date: r.published,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MinimalQuickAnswer {
    pub source: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl From<QuickAnswer> for MinimalQuickAnswer {
    fn from(qa: QuickAnswer) -> Self {
        Self {
            source: qa.source,
            text: qa.text,
            url: qa.url,
        }
    }
}

pub fn translate(query: String, resp: SearchResponse) -> SearchPayload {
    let count = resp.results.len();
    SearchPayload {
        query,
        count,
        results: resp.results.into_iter().map(MinimalResult::from).collect(),
        quick_answer: resp.quick_answer.map(MinimalQuickAnswer::from),
        related: resp.related_queries,
        total_results: resp.total_results,
        next_page: resp.next_page,
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct StatusInput {}

#[derive(Debug, Clone, Serialize)]
pub struct StatusResponse {
    pub session_found: bool,
    pub source: Option<String>,
    pub auth_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct FetchInput {
    /// The URL to fetch. Must be http/https. The Kagi session cookie is
    /// only sent to kagi.com hosts; third-party requests use just the UA.
    pub url: String,
    /// Maximum returned characters after readability extraction.
    /// Defaults to 50000.
    #[serde(default)]
    pub max_chars: Option<usize>,
    /// If true, return raw HTML instead of extracted readable text.
    #[serde(default)]
    pub raw: bool,
}

impl FetchInput {
    pub fn into_domain(self) -> FetchRequest {
        FetchRequest {
            url: self.url,
            max_chars: self.max_chars,
            raw: self.raw,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchPayload {
    pub url: String,
    pub final_url: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub body: String,
    pub truncated: bool,
}

impl From<FetchResponse> for FetchPayload {
    fn from(r: FetchResponse) -> Self {
        Self {
            url: r.url,
            final_url: r.final_url,
            status: r.status,
            content_type: r.content_type,
            title: r.title,
            body: r.body,
            truncated: r.truncated,
        }
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct WikipediaInput {
    /// Topic or entity to look up. Backed by /api/wikipedia.
    pub query: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WikipediaPayload {
    pub source: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl From<QuickAnswer> for WikipediaPayload {
    fn from(qa: QuickAnswer) -> Self {
        Self {
            source: qa.source,
            text: qa.text,
            url: qa.url,
        }
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SuggestInput {
    /// Partial query to autocomplete.
    pub query: String,
    /// Maximum suggestions to return. Defaults to 10.
    #[serde(default)]
    pub limit: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestPayload {
    pub query: String,
    pub count: usize,
    pub suggestions: Vec<MinimalSuggestion>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinimalSuggestion {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl From<Suggestion> for MinimalSuggestion {
    fn from(s: Suggestion) -> Self {
        Self {
            text: s.text,
            description: s.description,
            image_url: s.image_url,
            source: s.source,
        }
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct FastGptInput {
    /// The question to answer. Returns a synthesized answer with inline
    /// [N] citation markers keyed to the sources list.
    pub query: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FastGptPayload {
    pub query: String,
    pub answer: String,
    pub sources: Vec<MinimalFastGptSource>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinimalFastGptSource {
    pub n: u32,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

impl FastGptPayload {
    pub fn from_domain(query: String, ans: FastGptAnswer) -> Self {
        Self {
            query,
            answer: ans.answer,
            sources: ans
                .sources
                .into_iter()
                .enumerate()
                .map(|(i, s)| MinimalFastGptSource {
                    n: (i + 1) as u32,
                    title: s.title,
                    url: s.url,
                    snippet: s.snippet,
                })
                .collect(),
        }
    }
}

impl From<FastGptSource> for MinimalFastGptSource {
    fn from(_: FastGptSource) -> Self {
        unreachable!("use FastGptPayload::from_domain")
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct VerticalSearchInput {
    /// The search query.
    pub query: String,
    /// Maximum results. Defaults to 10.
    #[serde(default)]
    pub limit: Option<u8>,
    /// Recency filter: "day", "week", "month", "year".
    #[serde(default)]
    pub freshness: Option<String>,
    /// Sort order: 1 (relevance), 2 (recency), 3 (source ranking).
    #[serde(default)]
    pub order: Option<u8>,
    /// If true, sort descending instead of ascending.
    #[serde(default)]
    pub dir_desc: bool,
    /// AI-content filter (images/videos): "none" excludes AI-generated
    /// content; "only" includes only AI content.
    #[serde(default)]
    pub ai_filter: Option<String>,
    /// Image aspect: "square", "tall", "wide".
    #[serde(default)]
    pub aspect: Option<String>,
    /// Image color filter: "black", "blue", etc.
    #[serde(default)]
    pub color: Option<String>,
    /// Video duration: "short", "medium", "long".
    #[serde(default)]
    pub duration: Option<String>,
}

impl VerticalSearchInput {
    pub fn into_domain(self) -> VerticalQuery {
        VerticalQuery {
            q: self.query,
            limit: Some(self.limit.unwrap_or(10).clamp(1, 50)),
            freshness: self.freshness.as_deref().and_then(parse_time),
            order: self.order,
            dir_desc: self.dir_desc,
            ai_filter: self.ai_filter,
            aspect: self.aspect,
            color: self.color,
            duration: self.duration,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VerticalSearchPayload {
    pub vertical: String,
    pub query: String,
    pub count: usize,
    pub results: Vec<MinimalVerticalResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinimalVerticalResult {
    pub n: u32,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl From<VerticalResult> for MinimalVerticalResult {
    fn from(r: VerticalResult) -> Self {
        Self {
            n: r.rank,
            title: r.title,
            url: r.url,
            snippet: r.snippet,
            site: r.site,
            date: r.published,
            image_url: r.image_url,
            thumbnail_url: r.thumbnail_url,
            duration: r.duration,
            source: r.source,
        }
    }
}

pub fn translate_vertical(
    query: String,
    vertical: KagiVertical,
    resp: VerticalResponse,
) -> VerticalSearchPayload {
    let count = resp.results.len();
    VerticalSearchPayload {
        vertical: vertical.name().to_string(),
        query,
        count,
        results: resp
            .results
            .into_iter()
            .map(MinimalVerticalResult::from)
            .collect(),
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ListLensesInput {}

#[derive(Debug, Clone, Serialize)]
pub struct ListLensesPayload {
    pub count: usize,
    pub lenses: Vec<MinimalLens>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinimalLens {
    pub id: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub active: bool,
}

impl From<Lens> for MinimalLens {
    fn from(l: Lens) -> Self {
        Self {
            id: l.id,
            name: l.name,
            description: l.description,
            active: l.active,
        }
    }
}
