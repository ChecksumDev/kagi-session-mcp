use std::sync::Arc;

use crate::domain::{
    DomainResult, FastGptAnswer, FetchRequest, FetchResponse, KagiClient, KagiVertical, Lens,
    QuickAnswer, SearchQuery, SearchResponse, Suggestion, UrlFetcher, VerticalQuery,
    VerticalResponse,
};

use super::SessionDiscovery;

pub struct SearchService {
    discovery: Arc<SessionDiscovery>,
    client: Arc<dyn KagiClient>,
    fetcher: Arc<dyn UrlFetcher>,
}

impl SearchService {
    pub fn new(
        discovery: Arc<SessionDiscovery>,
        client: Arc<dyn KagiClient>,
        fetcher: Arc<dyn UrlFetcher>,
    ) -> Self {
        Self {
            discovery,
            client,
            fetcher,
        }
    }

    pub async fn search(&self, query: SearchQuery) -> DomainResult<SearchResponse> {
        let session = self.discovery.session().await?;
        self.client.search(&session, &query).await
    }

    pub async fn fetch(&self, request: FetchRequest) -> DomainResult<FetchResponse> {
        let session = self.discovery.session().await?;
        self.fetcher.fetch(&session, &request).await
    }

    pub async fn wikipedia(&self, query: &str) -> DomainResult<QuickAnswer> {
        let session = self.discovery.session().await?;
        self.client.wikipedia(&session, query).await
    }

    pub async fn suggest(&self, prefix: &str) -> DomainResult<Vec<Suggestion>> {
        let session = self.discovery.session().await?;
        self.client.suggest(&session, prefix).await
    }

    pub async fn fastgpt(&self, query: &str) -> DomainResult<FastGptAnswer> {
        let session = self.discovery.session().await?;
        self.client.fastgpt(&session, query).await
    }

    pub async fn vertical_search(
        &self,
        vertical: KagiVertical,
        query: VerticalQuery,
    ) -> DomainResult<VerticalResponse> {
        let session = self.discovery.session().await?;
        self.client
            .vertical_search(&session, vertical, &query)
            .await
    }

    pub async fn list_lenses(&self) -> DomainResult<Vec<Lens>> {
        let session = self.discovery.session().await?;
        self.client.list_lenses(&session).await
    }
}
