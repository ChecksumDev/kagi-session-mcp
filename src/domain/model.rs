use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum BrowserKind {
    Chrome,
    Edge,
    Brave,
    Arc,
    Vivaldi,
    Opera,
    OperaGx,
    Chromium,
    Firefox,
    Safari,
}

impl BrowserKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Edge => "edge",
            Self::Brave => "brave",
            Self::Arc => "arc",
            Self::Vivaldi => "vivaldi",
            Self::Opera => "opera",
            Self::OperaGx => "opera_gx",
            Self::Chromium => "chromium",
            Self::Firefox => "firefox",
            Self::Safari => "safari",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionAuth {
    Cookie { name: String, value: String },
    UrlToken(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub auth: SessionAuth,
    pub user_agent: String,
    pub source: BrowserKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum TimeRange {
    PastDay,
    PastWeek,
    PastMonth,
    PastYear,
}

impl TimeRange {
    pub const fn as_kagi(self) -> &'static str {
        match self {
            Self::PastDay => "d",
            Self::PastWeek => "w",
            Self::PastMonth => "m",
            Self::PastYear => "y",
        }
    }

    pub const fn as_freshness(self) -> &'static str {
        match self {
            Self::PastDay => "day",
            Self::PastWeek => "week",
            Self::PastMonth => "month",
            Self::PastYear => "year",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum SafeSearch {
    Off,
    Moderate,
    Strict,
}

impl SafeSearch {
    pub const fn as_kagi(self) -> &'static str {
        match self {
            Self::Off => "0",
            Self::Moderate => "1",
            Self::Strict => "2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<u8>,
    pub page: Option<u32>,
    pub time: Option<TimeRange>,
    pub region: Option<String>,
    pub safe: Option<SafeSearch>,
    pub lens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub rank: u32,
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub site: Option<String>,
    pub published: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub quick_answer: Option<QuickAnswer>,
    pub related_queries: Vec<String>,
    pub total_results: Option<u64>,
    pub next_page: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickAnswer {
    pub source: String,
    pub text: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchRequest {
    pub url: String,
    pub max_chars: Option<usize>,
    pub raw: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub text: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastGptAnswer {
    pub answer: String,
    pub sources: Vec<FastGptSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastGptSource {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalQuery {
    pub q: String,
    pub limit: Option<u8>,
    pub freshness: Option<TimeRange>,
    pub order: Option<u8>,
    pub dir_desc: bool,
    pub ai_filter: Option<String>,
    pub aspect: Option<String>,
    pub color: Option<String>,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalResult {
    pub rank: u32,
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub site: Option<String>,
    pub published: Option<String>,
    pub image_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub duration: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalResponse {
    pub results: Vec<VerticalResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lens {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResponse {
    pub url: String,
    pub final_url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub body: String,
    pub truncated: bool,
}
