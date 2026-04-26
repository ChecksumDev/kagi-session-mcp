pub mod chromium;
pub mod firefox;
#[cfg(target_os = "macos")]
pub mod safari;

use std::sync::Arc;

use crate::domain::{BrowserKind, SessionSource};

pub fn all_sources() -> Vec<Arc<dyn SessionSource>> {
    let mut v: Vec<Arc<dyn SessionSource>> = Vec::new();

    for kind in [
        BrowserKind::Chrome,
        BrowserKind::Edge,
        BrowserKind::Brave,
        BrowserKind::Arc,
        BrowserKind::Vivaldi,
        BrowserKind::Opera,
        BrowserKind::OperaGx,
        BrowserKind::Chromium,
    ] {
        if let Some(src) = chromium::ChromiumSource::for_kind(kind) {
            v.push(Arc::new(src));
        }
    }

    for src in firefox::all_firefox_like() {
        v.push(Arc::new(src));
    }

    #[cfg(target_os = "macos")]
    v.push(Arc::new(safari::SafariSource::new()));

    v
}
