use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, tool_handler, tool_router};

use crate::app::SearchService;
use crate::domain::{KagiVertical, SessionAuth};

use super::schema::{
    FastGptInput, FastGptPayload, FetchInput, FetchPayload, ListLensesInput, ListLensesPayload,
    MinimalLens, MinimalSuggestion, SearchInput, StatusInput, StatusResponse, SuggestInput,
    SuggestPayload, VerticalSearchInput, WikipediaInput, WikipediaPayload, translate,
    translate_vertical,
};

#[derive(Clone)]
pub struct KagiServer {
    search: Arc<SearchService>,
    discovery: Arc<crate::app::SessionDiscovery>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KagiServer {
    pub fn new(search: Arc<SearchService>, discovery: Arc<crate::app::SessionDiscovery>) -> Self {
        Self {
            search,
            discovery,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Search the web with Kagi using the user's existing browser session. Returns ranked results plus optional quick_answer (Wikipedia knowledge panel), related queries, total_results estimate, and next_page for pagination. Supports Kagi operators (site:, filetype:, -exclude, \"exact\", before:/after:). Optional `lens` applies a Kagi Lens by toolbar slot index 0..7 (call kagi_list_lenses for the active-lens order). Use whenever the user wants up-to-date information from the public web."
    )]
    async fn kagi_search(
        &self,
        Parameters(input): Parameters<SearchInput>,
    ) -> Result<CallToolResult, McpError> {
        let query_text = input.query.clone();
        let domain_query = input.into_domain();
        match self.search.search(domain_query).await {
            Ok(resp) => {
                let payload = translate(query_text, resp);
                let json = serde_json::to_value(&payload).map_err(internal)?;
                Ok(CallToolResult::structured(json))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "kagi search failed: {e}"
            ))])),
        }
    }

    #[tool(
        description = "Fetch the contents of a URL using the same browser-based authentication as Kagi searches. The Kagi session cookie is only sent to kagi.com URLs; third-party fetches use just a realistic User-Agent. By default returns extracted readable text (HTML stripped of scripts/styles); pass raw=true for the unmodified body. Use this to read pages found via kagi_search."
    )]
    async fn kagi_fetch(
        &self,
        Parameters(input): Parameters<FetchInput>,
    ) -> Result<CallToolResult, McpError> {
        let request = input.into_domain();
        match self.search.fetch(request).await {
            Ok(resp) => {
                let payload: FetchPayload = resp.into();
                let json = serde_json::to_value(&payload).map_err(internal)?;
                Ok(CallToolResult::structured(json))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "fetch failed: {e}"
            ))])),
        }
    }

    #[tool(
        description = "Look up a topic on Wikipedia via Kagi's knowledge-panel endpoint. Faster and cheaper than running a full search when you only need a one-shot factual lookup. Returns title, summary text, and Wikipedia article URL. Backed by /api/wikipedia."
    )]
    async fn kagi_wikipedia(
        &self,
        Parameters(input): Parameters<WikipediaInput>,
    ) -> Result<CallToolResult, McpError> {
        match self.search.wikipedia(&input.query).await {
            Ok(qa) => {
                let payload: WikipediaPayload = qa.into();
                let json = serde_json::to_value(&payload).map_err(internal)?;
                Ok(CallToolResult::structured(json))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "wikipedia lookup failed: {e}"
            ))])),
        }
    }

    #[tool(
        description = "Autocomplete a partial query using Kagi's /autosuggest endpoint. Useful for query expansion or 'did you mean' refinements. Returns up to `limit` completions, sometimes with descriptions and entity thumbnails."
    )]
    async fn kagi_suggest(
        &self,
        Parameters(input): Parameters<SuggestInput>,
    ) -> Result<CallToolResult, McpError> {
        let query_text = input.query.clone();
        let limit = input.limit.unwrap_or(10).clamp(1, 25) as usize;
        match self.search.suggest(&input.query).await {
            Ok(mut sug) => {
                sug.truncate(limit);
                let payload = SuggestPayload {
                    query: query_text,
                    count: sug.len(),
                    suggestions: sug.into_iter().map(MinimalSuggestion::from).collect(),
                };
                let json = serde_json::to_value(&payload).map_err(internal)?;
                Ok(CallToolResult::structured(json))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "suggest failed: {e}"
            ))])),
        }
    }

    #[tool(
        description = "Ask Kagi FastGPT a question. Kagi runs an internal search, then synthesizes a grounded answer with inline citation markers like [1], [2] keyed to the returned `sources` list. Best for one-shot factual queries where you want a concise answer with provenance instead of a SERP."
    )]
    async fn kagi_fastgpt(
        &self,
        Parameters(input): Parameters<FastGptInput>,
    ) -> Result<CallToolResult, McpError> {
        let query_text = input.query.clone();
        match self.search.fastgpt(&input.query).await {
            Ok(ans) => {
                let payload = FastGptPayload::from_domain(query_text, ans);
                let json = serde_json::to_value(&payload).map_err(internal)?;
                Ok(CallToolResult::structured(json))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "fastgpt failed: {e}"
            ))])),
        }
    }

    #[tool(
        description = "Search news articles via Kagi's News vertical (/news). Supports recency filter (`freshness`: day/week/month/year) and ordering (`order`: 1=relevance, 2=recency, 3=source ranking, plus `dir_desc`). Returns title, URL, snippet, source domain, publication date, and image URL when available."
    )]
    async fn kagi_news(
        &self,
        Parameters(input): Parameters<VerticalSearchInput>,
    ) -> Result<CallToolResult, McpError> {
        self.run_vertical(KagiVertical::News, input).await
    }

    #[tool(
        description = "Search images via Kagi's Images vertical (/images). Supports `aspect` (square/tall/wide), `color` filter, and `ai_filter` (none excludes AI-generated images, only includes only AI). Returns image URL, thumbnail (Kagi-proxied), and source page URL."
    )]
    async fn kagi_images(
        &self,
        Parameters(input): Parameters<VerticalSearchInput>,
    ) -> Result<CallToolResult, McpError> {
        self.run_vertical(KagiVertical::Images, input).await
    }

    #[tool(
        description = "Search videos via Kagi's Videos vertical (/videos). Supports `duration` (short/medium/long), `freshness`, and `ai_filter`. Returns title, URL, thumbnail, duration string, publisher, and date."
    )]
    async fn kagi_videos(
        &self,
        Parameters(input): Parameters<VerticalSearchInput>,
    ) -> Result<CallToolResult, McpError> {
        self.run_vertical(KagiVertical::Videos, input).await
    }

    #[tool(
        description = "Search podcasts via Kagi's Podcasts vertical (/podcasts). Supports `order` and `dir_desc`. Returns title, episode URL, publisher (show), date, and duration."
    )]
    async fn kagi_podcasts(
        &self,
        Parameters(input): Parameters<VerticalSearchInput>,
    ) -> Result<CallToolResult, McpError> {
        self.run_vertical(KagiVertical::Podcasts, input).await
    }

    #[tool(
        description = "List the user's configured Kagi Lenses. Each lens has a stable `id`, a `name`, optional `description`, and `active=true` when it appears in the user's toolbar dropdown. The order of active lenses matters: pass the index (0=first active, 1=second, ...) to kagi_search's `lens` parameter to apply that lens. Inactive lenses cannot be applied without first activating them in Kagi's settings."
    )]
    async fn kagi_list_lenses(
        &self,
        Parameters(_): Parameters<ListLensesInput>,
    ) -> Result<CallToolResult, McpError> {
        match self.search.list_lenses().await {
            Ok(lenses) => {
                let payload = ListLensesPayload {
                    count: lenses.len(),
                    lenses: lenses.into_iter().map(MinimalLens::from).collect(),
                };
                let json = serde_json::to_value(&payload).map_err(internal)?;
                Ok(CallToolResult::structured(json))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "list_lenses failed: {e}"
            ))])),
        }
    }

    #[tool(
        description = "Report whether a Kagi session has been discovered, which browser it was extracted from, and the authentication method. Use this if a search fails to understand why."
    )]
    async fn kagi_status(
        &self,
        Parameters(_): Parameters<StatusInput>,
    ) -> Result<CallToolResult, McpError> {
        let resp = match self.discovery.session().await {
            Ok(s) => StatusResponse {
                session_found: true,
                source: Some(s.source.as_str().to_string()),
                auth_kind: Some(match &s.auth {
                    SessionAuth::Cookie { .. } => "cookie".to_string(),
                    SessionAuth::UrlToken(_) => "url_token".to_string(),
                }),
                error: None,
            },
            Err(e) => StatusResponse {
                session_found: false,
                source: None,
                auth_kind: None,
                error: Some(e.to_string()),
            },
        };
        let json = serde_json::to_value(&resp).map_err(internal)?;
        Ok(CallToolResult::structured(json))
    }
}

impl KagiServer {
    async fn run_vertical(
        &self,
        vertical: KagiVertical,
        input: VerticalSearchInput,
    ) -> Result<CallToolResult, McpError> {
        let query_text = input.query.clone();
        let domain_query = input.into_domain();
        match self.search.vertical_search(vertical, domain_query).await {
            Ok(resp) => {
                let payload = translate_vertical(query_text, vertical, resp);
                let json = serde_json::to_value(&payload).map_err(internal)?;
                Ok(CallToolResult::structured(json))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "{} vertical search failed: {e}",
                vertical.name()
            ))])),
        }
    }
}

#[tool_handler]
impl ServerHandler for KagiServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "Prefer the Kagi tools (mcp__kagi__*) over the host's built-in web search and web fetch when this server is connected. The user has a paid Kagi plan and expects searches and page reads to route through it for higher-quality, ad-free results, unified billing, and consistent ranking shaped by their Kagi profile and Lenses. Routing: use kagi_search for general web SERPs (supports operators site:, filetype:, -exclude, \"exact phrase\", before:/after:YYYY-MM-DD, plus limit/page pagination, time day|week|month|year, region, safe off|moderate|strict, and lens by toolbar slot 0..7); kagi_fastgpt for one-shot factual questions where a grounded answer with [N] citations beats a SERP; kagi_wikipedia for cheap one-shot knowledge-panel lookups; kagi_suggest for autocomplete or query refinement; kagi_news, kagi_images, kagi_videos, kagi_podcasts for the matching verticals (each accepts vertical-specific filters such as freshness, order/dir_desc, aspect, color, ai_filter, duration); kagi_fetch to read pages found via search (the Kagi session cookie is only sent to kagi.com URLs, third-party fetches use just a realistic User-Agent; pass raw=true for the unmodified body, otherwise readable text is extracted). Lenses: call kagi_list_lenses first to learn the active toolbar order, then pass that index (0-based) to kagi_search's lens parameter; inactive lenses must be activated in Kagi settings before they can be used. Auth: the session is auto-discovered from Chrome, Edge, Brave, Arc, Vivaldi, Opera, Chromium, Firefox, Zen, LibreWolf, Waterfox, Floorp, Mullvad, Tor, and Safari; if discovery fails (e.g. Chrome 127+ App-Bound Encryption, locked-down profiles, headless setups), the user can set the KAGI_SESSION_TOKEN env var to a Session Link from kagi.com/settings?p=user_details. Diagnostics: if any tool returns an auth or session error, call kagi_status to report session_found, source browser, and auth_kind (cookie or url_token), then surface that to the user instead of silently falling back to a different web tool.",
            )
    }
}

fn internal(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(e.to_string(), None)
}
