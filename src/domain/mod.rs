pub mod error;
pub mod model;
pub mod ports;

pub use error::{DomainError, DomainResult};
pub use model::{
    BrowserKind, FastGptAnswer, FastGptSource, FetchRequest, FetchResponse, Lens, QuickAnswer,
    SafeSearch, SearchQuery, SearchResponse, SearchResult, Session, SessionAuth, Suggestion,
    TimeRange, VerticalQuery, VerticalResponse, VerticalResult,
};
pub use ports::{KagiClient, KagiVertical, SessionSource, UrlFetcher};
