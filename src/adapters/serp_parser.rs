use scraper::{ElementRef, Html, Selector};

use crate::domain::{
    DomainResult, FastGptAnswer, FastGptSource, KagiVertical, Lens, QuickAnswer, SearchResult,
    VerticalResult,
};

// Result-row containers carry both `_0_SRI` and `search-result`. Image-strip
// footers also use `_0_SRI`; we exclude those at extract time.
static RESULT_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse("div._0_SRI.search-result, div.search-result._0_SRI").unwrap()
});
static TITLE_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse(
        ".__sri-title a, .__sri_title_box a, .__sri-title-box a, h3 a, a.__sri-title, a._0_sri_title_link, a.__sri_title_link",
    )
    .unwrap()
});
static SNIPPET_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse(".__sri-desc, .__sri_desc, .__sri-body, .__sri_body, ._0_DESC").unwrap()
});
static URL_BOX_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse(".__sri-url-box a, .__sri_url_box a, .__sri-url a, ._0_URL a").unwrap()
});
static SITE_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse(".__sri-site, .__sri_site, .domain, .__sri-url-box, .__sri_url_path_box")
        .unwrap()
});
static TIME_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse(".__sri_age, .__sri-age, .__sri-time, .__result-date, time").unwrap()
});

pub fn parse_serp(html: &str) -> DomainResult<Vec<SearchResult>> {
    let doc = Html::parse_document(html);
    let mut out = Vec::new();
    let mut next_rank = 1u32;
    for el in doc.select(&RESULT_SEL) {
        let class_attr = el.value().attr("class").unwrap_or("");
        if class_attr.contains("image-preview-footer") {
            continue;
        }
        if let Some(mut r) = extract_one(&el, next_rank) {
            r.rank = next_rank;
            next_rank += 1;
            out.push(r);
        }
    }
    Ok(out)
}

fn extract_one(el: &ElementRef<'_>, rank: u32) -> Option<SearchResult> {
    let title_a = el.select(&TITLE_SEL).next()?;
    let title = clean_text(&title_a.text().collect::<String>());
    let url_from_title = title_a.value().attr("href").unwrap_or_default().to_string();

    let url = if url_from_title.starts_with("http") {
        url_from_title
    } else {
        el.select(&URL_BOX_SEL)
            .next()
            .and_then(|a| a.value().attr("href"))
            .unwrap_or_default()
            .to_string()
    };

    if title.is_empty() || url.is_empty() {
        return None;
    }

    let snippet = el
        .select(&SNIPPET_SEL)
        .next()
        .map(|s| clean_text(&s.text().collect::<String>()))
        .filter(|s| !s.is_empty());

    let site = el
        .select(&SITE_SEL)
        .next()
        .map(|s| clean_text(&s.text().collect::<String>()))
        .filter(|s| !s.is_empty());

    let published = el
        .select(&TIME_SEL)
        .next()
        .map(|s| clean_text(&s.text().collect::<String>()))
        .filter(|s| !s.is_empty());

    Some(SearchResult {
        rank,
        title,
        url,
        snippet,
        site,
        published,
    })
}

pub fn parse_wikipedia_panel(html: &str) -> Option<QuickAnswer> {
    static URL_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
        Selector::parse(".wikipediaContent[data-current_article]").unwrap()
    });
    static BODY_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
        Selector::parse(".wikipediaContent, .wikipediaResult").unwrap()
    });
    // Chrome elements (outbound link, expand toggle) get pruned before text collection.
    static CHROME_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
        Selector::parse(".wiki-nw-link, .smw, .cardHistory, ._0_cardHistoryBack").unwrap()
    });

    let doc = Html::parse_document(html);
    let url = doc
        .select(&URL_SEL)
        .next()
        .and_then(|el| el.value().attr("data-current_article"))
        .map(ToString::to_string);
    let root = doc.select(&BODY_SEL).next()?;

    let mut excluded: std::collections::HashSet<ego_tree::NodeId> =
        std::collections::HashSet::new();
    for chrome in root.select(&CHROME_SEL) {
        for d in chrome.descendants() {
            excluded.insert(d.id());
        }
        excluded.insert(chrome.id());
    }

    let mut buf = String::new();
    for d in root.descendants() {
        if excluded.contains(&d.id()) {
            continue;
        }
        if let scraper::Node::Text(t) = d.value() {
            buf.push_str(t);
            buf.push(' ');
        }
    }
    let body = clean_text(&buf);
    if body.is_empty() {
        return None;
    }
    Some(QuickAnswer {
        source: "wikipedia".to_string(),
        text: truncate_chars(&body, 1500),
        url,
    })
}

pub fn parse_related_searches(html: &str) -> Vec<String> {
    static A_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("a, button, span.related_query").unwrap());
    let doc = Html::parse_fragment(html);
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for el in doc.select(&A_SEL) {
        let t = clean_text(&el.text().collect::<String>());
        if t.is_empty() || t.len() > 200 {
            continue;
        }
        if seen.insert(t.clone()) {
            out.push(t);
        }
    }
    out
}

// Kagi formats this chunk as "<i>71</i> relevant results in <i>0.54s</i>".
pub fn parse_top_content_unique(html: &str) -> Option<u64> {
    static I_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("i").unwrap());
    let doc = Html::parse_fragment(html);
    let first = doc.select(&I_SEL).next()?;
    let txt = first.text().collect::<String>();
    let cleaned: String = txt.chars().filter(char::is_ascii_digit).collect();
    cleaned.parse().ok()
}

// Sources live in <details>; the answer body lives after </details> with
// <sup><a>N</a></sup> citation markers we rewrite as [N] inline.
pub fn parse_fastgpt_answer(html: &str) -> FastGptAnswer {
    static SOURCE_BLOCK_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("details").unwrap());
    static STRONG_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("strong").unwrap());
    static A_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("a").unwrap());

    let doc = Html::parse_fragment(html);

    let mut sources: Vec<FastGptSource> = Vec::new();
    if let Some(details) = doc.select(&SOURCE_BLOCK_SEL).next() {
        let strongs: Vec<_> = details.select(&STRONG_SEL).collect();
        let anchors: Vec<_> = details.select(&A_SEL).collect();
        for (i, (s, a)) in strongs.iter().zip(anchors.iter()).enumerate() {
            let title = clean_text(&s.text().collect::<String>());
            let url = a.value().attr("href").unwrap_or_default().to_string();
            if title.is_empty() || url.is_empty() {
                continue;
            }
            let next_strong_pos = strongs.get(i + 1).map(|s| s.id());
            let snippet = text_until_next_strong(a, next_strong_pos);
            sources.push(FastGptSource {
                title,
                url,
                snippet: Some(truncate_chars(&snippet, 500)).filter(|s| !s.is_empty()),
            });
        }
    }

    let answer_html = html
        .find("</details>")
        .map_or(html, |end| &html[end + "</details>".len()..]);
    let answer = render_answer_text(answer_html);

    FastGptAnswer { answer, sources }
}

fn render_answer_text(html: &str) -> String {
    static SUP_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"<sup[^>]*>\s*<a[^>]*>\s*(\d+)\s*</a>\s*</sup>").unwrap()
    });
    static BLOCK_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"</(?:p|li|h[1-6]|div|br)>").unwrap());
    static TAG_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"<[^>]+>").unwrap());

    let s = SUP_RE.replace_all(html, "[$1]");
    let s = BLOCK_RE.replace_all(&s, "\n");
    let s = TAG_RE.replace_all(&s, "");
    let s = s
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ");
    s.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn text_until_next_strong(anchor: &ElementRef<'_>, stop_at_id: Option<ego_tree::NodeId>) -> String {
    let mut out = String::new();
    let mut current = anchor.next_sibling();
    let mut ancestor_iter = anchor.ancestors();
    let _ = ancestor_iter.next();
    loop {
        while let Some(n) = current {
            if Some(n.id()) == stop_at_id {
                return clean_text(&out);
            }
            if let Some(el) = ElementRef::wrap(n) {
                if el.value().name() == "strong" {
                    return clean_text(&out);
                }
                for t in el.text() {
                    out.push_str(t);
                    out.push(' ');
                }
                if let Some(stop) = stop_at_id
                    && el.descendants().any(|d| d.id() == stop)
                {
                    return clean_text(&out);
                }
            } else if let scraper::Node::Text(t) = n.value() {
                out.push_str(t);
            }
            current = n.next_sibling();
        }
        match ancestor_iter.next() {
            Some(parent) => current = parent.next_sibling(),
            None => break,
        }
    }
    clean_text(&out)
}

pub fn parse_vertical_serp(
    html: &str,
    vertical: KagiVertical,
) -> DomainResult<Vec<VerticalResult>> {
    let doc = Html::parse_document(html);
    let results = match vertical {
        KagiVertical::News => parse_news(&doc),
        KagiVertical::Videos => parse_videos(&doc),
        KagiVertical::Podcasts => parse_podcasts(&doc),
        KagiVertical::Images => parse_images(&doc),
    };
    Ok(results)
}

fn parse_news(doc: &Html) -> Vec<VerticalResult> {
    static ITEM_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse(".newsResultItem._0_SRI").unwrap());
    static TITLE_A_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
        Selector::parse("h3.__sri-title-box a, .newsResultTitle a").unwrap()
    });
    static BODY_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
        Selector::parse(".newsResultBody, .newsResultContent").unwrap()
    });
    static TIME_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse(".newsResultTime").unwrap());
    static IMG_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse(".newsResultImage img").unwrap());

    let mut out = Vec::new();
    for (i, el) in doc.select(&ITEM_SEL).enumerate() {
        let Some(a) = el.select(&TITLE_A_SEL).next() else {
            continue;
        };
        let title = clean_text(&a.text().collect::<String>());
        let url = a.value().attr("href").unwrap_or_default().to_string();
        if title.is_empty() || url.is_empty() {
            continue;
        }
        let snippet = el
            .select(&BODY_SEL)
            .next()
            .map(|s| clean_text(&s.text().collect::<String>()))
            .filter(|s| !s.is_empty());
        let published = el
            .select(&TIME_SEL)
            .next()
            .map(|s| clean_text(&s.text().collect::<String>()))
            .filter(|s| !s.is_empty());
        let site = a.value().attr("data-domain").map(ToString::to_string);
        let image_url = el
            .select(&IMG_SEL)
            .next()
            .and_then(|i| i.value().attr("src"))
            .map(ToString::to_string);
        out.push(VerticalResult {
            rank: (i + 1) as u32,
            title,
            url,
            snippet,
            site,
            published,
            image_url,
            thumbnail_url: None,
            duration: None,
            source: None,
        });
    }
    out
}

fn parse_videos(doc: &Html) -> Vec<VerticalResult> {
    static ITEM_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse(".videoResultItem._0_SRI").unwrap());
    static TITLE_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("a.videoResultTitle").unwrap());
    static THUMB_A_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("a.videoResultThumbnail").unwrap());
    static THUMB_IMG_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("a.videoResultThumbnail img").unwrap());
    static DUR_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse(".videoResultVideoTime").unwrap());
    static PUBLISHER_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("a.videoPublisher").unwrap());
    static DATE_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse(".videoPublishedDate").unwrap());

    let mut out = Vec::new();
    for (i, el) in doc.select(&ITEM_SEL).enumerate() {
        // Falls back to thumbnail alt text when the title anchor is absent.
        let (title, url) = if let Some(a) = el.select(&TITLE_SEL).next() {
            let t = clean_text(&a.text().collect::<String>());
            let u = a.value().attr("href").unwrap_or_default().to_string();
            (t, u)
        } else if let Some(a) = el.select(&THUMB_A_SEL).next() {
            let u = a.value().attr("href").unwrap_or_default().to_string();
            let t = el
                .select(&THUMB_IMG_SEL)
                .next()
                .and_then(|i| i.value().attr("alt"))
                .unwrap_or("")
                .trim_start_matches("Video Thumbnail of ")
                .to_string();
            (t, u)
        } else {
            continue;
        };
        if title.is_empty() || url.is_empty() {
            continue;
        }
        let thumbnail_url = el
            .select(&THUMB_IMG_SEL)
            .next()
            .and_then(|i| i.value().attr("src"))
            .map(ToString::to_string);
        let duration = el
            .select(&DUR_SEL)
            .next()
            .map(|s| clean_text(&s.text().collect::<String>()))
            .filter(|s| !s.is_empty());
        let source = el
            .select(&PUBLISHER_SEL)
            .next()
            .and_then(|a| {
                a.value()
                    .attr("aria-label")
                    .and_then(|s| s.strip_prefix("Video publisher "))
                    .map(ToString::to_string)
                    .or_else(|| Some(clean_text(&a.text().collect::<String>())))
            })
            .filter(|s| !s.is_empty());
        let published = el
            .select(&DATE_SEL)
            .next()
            .map(|s| clean_text(&s.text().collect::<String>()))
            .filter(|s| !s.is_empty());
        out.push(VerticalResult {
            rank: (i + 1) as u32,
            title,
            url,
            snippet: None,
            site: None,
            published,
            image_url: None,
            thumbnail_url,
            duration,
            source,
        });
    }
    out
}

fn parse_podcasts(doc: &Html) -> Vec<VerticalResult> {
    // Kagi's podcast vertical is a "Top Shows" carousel; no episode-level data.
    static ITEM_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("a.top_media_item").unwrap());
    static TITLE_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse(".top_media_item_title").unwrap());
    static THUMB_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("img.top_media_item_thumb, img").unwrap());

    let mut out = Vec::new();
    for (i, el) in doc.select(&ITEM_SEL).enumerate() {
        let url = el.value().attr("href").unwrap_or_default().to_string();
        if url.is_empty() {
            continue;
        }
        let title = el
            .select(&TITLE_SEL)
            .next()
            .map(|t| clean_text(&t.text().collect::<String>()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("Podcast {}", i + 1));
        let thumbnail_url = el
            .select(&THUMB_SEL)
            .next()
            .and_then(|i| i.value().attr("src"))
            .map(ToString::to_string);
        out.push(VerticalResult {
            rank: (i + 1) as u32,
            title,
            url,
            snippet: None,
            site: None,
            published: None,
            image_url: None,
            thumbnail_url,
            duration: None,
            source: None,
        });
    }
    out
}

fn parse_images(doc: &Html) -> Vec<VerticalResult> {
    // Image items carry metadata as data-* attrs. data-host_url is the source
    // page; the inner `a._0_img_link_el` href is the proxied full image.
    static ITEM_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("div._0_image_item").unwrap());
    static LINK_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("a._0_img_link_el").unwrap());
    static THUMB_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("img._0_img_src, img").unwrap());

    let mut out = Vec::new();
    for (i, el) in doc.select(&ITEM_SEL).enumerate() {
        let attr = |k| el.value().attr(k).map(ToString::to_string);
        let title = attr("data-title").unwrap_or_else(|| format!("Image {}", i + 1));
        let url = attr("data-host_url").unwrap_or_default();
        if url.is_empty() {
            continue;
        }
        let image_url = el
            .select(&LINK_SEL)
            .next()
            .and_then(|a| a.value().attr("href"))
            .map(ToString::to_string)
            .or_else(|| attr("data-content_url"));
        let thumbnail_url = el
            .select(&THUMB_SEL)
            .next()
            .and_then(|i| i.value().attr("src"))
            .map(ToString::to_string);
        let site = attr("data-host");
        let published = attr("data-date_published").filter(|s| !s.is_empty());
        let snippet = match (attr("data-width"), attr("data-height")) {
            (Some(w), Some(h)) if !w.is_empty() && !h.is_empty() => Some(format!("{w}x{h}")),
            _ => None,
        };
        out.push(VerticalResult {
            rank: (i + 1) as u32,
            title,
            url,
            snippet,
            site,
            published,
            image_url,
            thumbnail_url,
            duration: None,
            source: None,
        });
    }
    out
}

// Lens rows live at div.__lens_item[data-id="N"]. Active lenses are scoped
// to #_0_lens_table_active; everything else is "Other Lenses".
pub fn parse_lens_settings(html: &str) -> Vec<Lens> {
    static ITEM_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
        Selector::parse(".__lens_item[data-id], ._0_lens_item[data-id]").unwrap()
    });
    static NAME_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse(".lens_title > div:not(.desc_box)").unwrap());
    static DESC_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse(".lens_desc, .lens_label_desc").unwrap());
    static ACTIVE_TABLE_SEL: std::sync::LazyLock<Selector> =
        std::sync::LazyLock::new(|| Selector::parse("#_0_lens_table_active").unwrap());

    let doc = Html::parse_document(html);
    let mut active_ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
    if let Some(active_table) = doc.select(&ACTIVE_TABLE_SEL).next() {
        for el in active_table.select(&ITEM_SEL) {
            if let Some(id_str) = el.value().attr("data-id")
                && let Ok(id) = id_str.parse::<u32>()
            {
                active_ids.insert(id);
            }
        }
    }

    let mut out: Vec<Lens> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for el in doc.select(&ITEM_SEL) {
        let Some(id_str) = el.value().attr("data-id") else {
            continue;
        };
        let Ok(id) = id_str.parse::<u32>() else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        let name = el
            .select(&NAME_SEL)
            .next()
            .map(|n| clean_text(&n.text().collect::<String>()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("Lens #{id}"));
        let description = el
            .select(&DESC_SEL)
            .next()
            .map(|d| clean_text(&d.text().collect::<String>()))
            .filter(|s| !s.is_empty() && s != &name);
        out.push(Lens {
            id,
            name,
            description,
            active: active_ids.contains(&id),
        });
    }
    out
}

fn clean_text(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
