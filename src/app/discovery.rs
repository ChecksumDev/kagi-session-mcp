use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::domain::{DomainError, DomainResult, Session, SessionAuth, SessionSource};

pub struct SessionDiscovery {
    sources: Vec<Arc<dyn SessionSource>>,
    manual_token: Option<String>,
    cache: Mutex<CacheEntry>,
}

const NEGATIVE_TTL: Duration = Duration::from_secs(300);

enum CacheEntry {
    Empty,
    Session(Arc<Session>),
    Failed { at: Instant, error: String },
}

impl SessionDiscovery {
    pub fn new(sources: Vec<Arc<dyn SessionSource>>, manual_token: Option<String>) -> Self {
        Self {
            sources,
            manual_token,
            cache: Mutex::new(CacheEntry::Empty),
        }
    }

    // Lock is intentionally held across the discover() call: two concurrent
    // tool invocations should share one cookie-DB probe, not race on it.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn session(&self) -> DomainResult<Arc<Session>> {
        let mut guard = self.cache.lock().await;
        match &*guard {
            CacheEntry::Session(s) => return Ok(s.clone()),
            CacheEntry::Failed { at, error } if at.elapsed() < NEGATIVE_TTL => {
                return Err(DomainError::SessionRejected(error.clone()));
            }
            _ => {}
        }
        match self.discover().await {
            Ok(s) => {
                let arc = Arc::new(s);
                *guard = CacheEntry::Session(arc.clone());
                Ok(arc)
            }
            Err(e) => {
                *guard = CacheEntry::Failed {
                    at: Instant::now(),
                    error: e.to_string(),
                };
                Err(e)
            }
        }
    }

    async fn discover(&self) -> DomainResult<Session> {
        if let Some(token) = &self.manual_token {
            tracing::info!("using manually configured kagi session token");
            return Ok(Session {
                auth: SessionAuth::UrlToken(token.clone()),
                user_agent: default_user_agent(),
                source: crate::domain::BrowserKind::Chrome,
            });
        }

        let mut last_err: Option<DomainError> = None;
        for src in &self.sources {
            if !src.is_available().await {
                continue;
            }
            tracing::debug!(browser = src.name(), "probing browser for kagi session");
            match src.extract().await {
                Ok(Some(s)) => {
                    tracing::info!(browser = src.name(), "found kagi session");
                    return Ok(s);
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(browser = src.name(), error = %e, "session extraction failed");
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or(DomainError::NoSessionFound))
    }
}

fn default_user_agent() -> String {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string()
}
