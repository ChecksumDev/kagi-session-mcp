use async_trait::async_trait;

use super::{
    DomainResult, FastGptAnswer, FetchRequest, FetchResponse, Lens, QuickAnswer, SearchQuery,
    SearchResponse, Session, Suggestion, VerticalQuery, VerticalResponse,
};

#[async_trait]
pub trait SessionSource: Send + Sync {
    fn name(&self) -> &'static str;

    async fn is_available(&self) -> bool;

    async fn extract(&self) -> DomainResult<Option<Session>>;
}

#[async_trait]
pub trait KagiClient: Send + Sync {
    async fn search(&self, session: &Session, query: &SearchQuery) -> DomainResult<SearchResponse>;

    async fn wikipedia(&self, session: &Session, query: &str) -> DomainResult<QuickAnswer>;

    async fn suggest(&self, session: &Session, prefix: &str) -> DomainResult<Vec<Suggestion>>;

    async fn fastgpt(&self, session: &Session, query: &str) -> DomainResult<FastGptAnswer>;

    async fn vertical_search(
        &self,
        session: &Session,
        vertical: KagiVertical,
        query: &VerticalQuery,
    ) -> DomainResult<VerticalResponse>;

    async fn list_lenses(&self, session: &Session) -> DomainResult<Vec<Lens>>;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum KagiVertical {
    News,
    Images,
    Videos,
    Podcasts,
}

impl KagiVertical {
    pub const fn path(self) -> &'static str {
        match self {
            Self::News => "/news",
            Self::Images => "/images",
            Self::Videos => "/videos",
            Self::Podcasts => "/podcasts",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::News => "news",
            Self::Images => "images",
            Self::Videos => "videos",
            Self::Podcasts => "podcasts",
        }
    }
}

#[async_trait]
pub trait UrlFetcher: Send + Sync {
    async fn fetch(&self, session: &Session, request: &FetchRequest)
    -> DomainResult<FetchResponse>;
}
