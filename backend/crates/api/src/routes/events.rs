//! `GET /api/v1/events`: kalendarz wydarzeń Fundacji IGTSF (spec 2026-10-06,
//! zgoda fundacji 06.10).
//!
//! Serwer pobiera publiczne REST The Events Calendar z igtsf.com, mapuje do
//! małego, płaskiego kształtu i trzyma wynik w pamięci 30 min. Źródło jest
//! zewnętrzne, więc endpoint NIGDY nie zwraca błędu z powodu IGTSF: przy
//! awarii/timeoucie oddaje ostatni cache albo pustą listę (i ponawia próbę po
//! kilku minutach, żeby nie odpytywać ich przy każdym żądaniu).
//!
//! Bez zapisu w bazie. Tekst czyszczony z HTML; linki tylko `https://`
//! (żadnych `javascript:` w `href` po stronie apki).

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use axum::Json;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{auth::extractor::AuthUser, error::AppError, response};

const SOURCE_URL: &str = "https://igtsf.com/wp-json/tribe/events/v1/events";
const CACHE_TTL: Duration = Duration::from_secs(30 * 60);
/// Po nieudanym pobraniu: kolejna próba najwcześniej po tym czasie.
const RETRY_AFTER_ERROR: Duration = Duration::from_secs(5 * 60);
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_EVENTS: usize = 50;
const DESCRIPTION_MAX_CHARS: usize = 300;

/// One upcoming event, as returned to the app.
///
/// `start` / `end` are local Europe/Warsaw wall-clock times without offset
/// (`YYYY-MM-DDTHH:MM:SS`), exactly as the organiser entered them.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CalendarEvent {
    pub id: i64,
    pub title: String,
    pub start: String,
    pub end: Option<String>,
    pub all_day: bool,
    pub venue: Option<String>,
    pub city: Option<String>,
    pub url: Option<String>,
    pub image: Option<String>,
    pub description: String,
}

struct Cache {
    events: Vec<CalendarEvent>,
    refresh_at: Instant,
}

static CACHE: LazyLock<Mutex<Option<Cache>>> = LazyLock::new(|| Mutex::new(None));

/// `GET /api/v1/events`
///
/// Upcoming IGTSF events (start >= today, max 50). Authenticated.
/// Returns 200 with `{ data: [CalendarEvent] }`, possibly empty.
pub async fn list_events(_auth: AuthUser) -> Result<Json<Value>, AppError> {
    Ok(response::data(cached_events().await))
}

async fn cached_events() -> Vec<CalendarEvent> {
    // tokio Mutex trzymany przez fetch: równoległe żądania czekają na jeden
    // odczyt z IGTSF (max FETCH_TIMEOUT) zamiast odpalać własne.
    let mut guard = CACHE.lock().await;
    let now = Instant::now();
    if let Some(cache) = guard.as_ref() {
        if now < cache.refresh_at {
            return cache.events.clone();
        }
    }
    match fetch_events().await {
        Ok(events) => {
            *guard = Some(Cache {
                events: events.clone(),
                refresh_at: now + CACHE_TTL,
            });
            events
        }
        Err(err) => {
            tracing::warn!(error = %err, "igtsf calendar fetch failed, serving last cache");
            let events = guard.as_ref().map(|c| c.events.clone()).unwrap_or_default();
            *guard = Some(Cache {
                events: events.clone(),
                refresh_at: now + RETRY_AFTER_ERROR,
            });
            events
        }
    }
}

async fn fetch_events() -> Result<Vec<CalendarEvent>, String> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let per_page = MAX_EVENTS.to_string();
    let resp = crate::util::http::client()
        .get(SOURCE_URL)
        .query(&[("start_date", today.as_str()), ("per_page", per_page.as_str())])
        .header(reqwest::header::ACCEPT, "application/json")
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("igtsf events: {}", e.without_url()))?;
    if !resp.status().is_success() {
        return Err(format!("igtsf events: HTTP {}", resp.status()));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("igtsf events: bad body: {}", e.without_url()))?;
    Ok(parse_events(&body))
}

/// Map the The Events Calendar response (`{ events: [...] }`) to our shape.
/// Malformed entries are skipped, never fatal.
fn parse_events(body: &Value) -> Vec<CalendarEvent> {
    body.get("events")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(map_event).take(MAX_EVENTS).collect())
        .unwrap_or_default()
}

fn map_event(e: &Value) -> Option<CalendarEvent> {
    if e.get("hide_from_listings").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let id = e.get("id")?.as_i64()?;
    let title = clean_text(e.get("title")?.as_str()?);
    if title.is_empty() {
        return None;
    }
    let start = normalize_datetime(e.get("start_date")?.as_str()?)?;
    let end = e
        .get("end_date")
        .and_then(Value::as_str)
        .and_then(normalize_datetime);
    let all_day = e.get("all_day").and_then(Value::as_bool).unwrap_or(false);

    // `venue` bywa obiektem albo pustą tablicą `[]` (wydarzenia online).
    let venue_obj = e.get("venue").and_then(Value::as_object);
    let venue_field = |key: &str| {
        venue_obj
            .and_then(|v| v.get(key))
            .and_then(Value::as_str)
            .map(clean_text)
            .filter(|s| !s.is_empty())
    };
    let venue = venue_field("venue");
    let city = venue_field("city");

    let url = e.get("url").and_then(Value::as_str).and_then(safe_https);
    // `image` bywa obiektem albo `false`.
    let image = e
        .get("image")
        .and_then(Value::as_object)
        .and_then(|i| i.get("url"))
        .and_then(Value::as_str)
        .and_then(safe_https);
    let description = truncate_chars(
        &clean_text(e.get("description").and_then(Value::as_str).unwrap_or("")),
        DESCRIPTION_MAX_CHARS,
    );

    Some(CalendarEvent {
        id,
        title,
        start,
        end,
        all_day,
        venue,
        city,
        url,
        image,
        description,
    })
}

/// `"2026-10-06 09:00:00"` -> `"2026-10-06T09:00:00"`; anything else -> `None`.
fn normalize_datetime(s: &str) -> Option<String> {
    chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
}

/// Only absolute `https://` URLs without whitespace pass (used in `href`/`src`).
fn safe_https(s: &str) -> Option<String> {
    let s = s.trim();
    let ok = s.len() > "https://".len()
        && s.len() <= 2048
        && s.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("https://"))
        && !s.chars().any(|c| c.is_whitespace() || c.is_control());
    ok.then(|| s.to_string())
}

/// Strip HTML (incl. `<script>`/`<style>` bodies), decode entities, collapse
/// whitespace.
fn clean_text(html: &str) -> String {
    let without_blocks = remove_block(&remove_block(html, "script"), "style");
    let mut text = String::with_capacity(without_blocks.len());
    let mut in_tag = false;
    for c in without_blocks.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    decode_entities(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Remove `<tag ...>...</tag>` blocks (ASCII case-insensitive).
fn remove_block(s: &str, tag: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::with_capacity(s.len());
    let mut pos = 0;
    while let Some(rel) = lower[pos..].find(&open) {
        let start = pos + rel;
        out.push_str(&s[pos..start]);
        match lower[start..].find(&close) {
            Some(end_rel) => pos = start + end_rel + close.len(),
            None => {
                pos = s.len();
                break;
            }
        }
    }
    out.push_str(&s[pos..]);
    out
}

/// Decode numeric (`&#8211;`, `&#x2013;`) and common named entities.
/// Unknown entities stay as written.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let decoded = after
            .char_indices()
            .take(12)
            .find(|&(_, c)| c == ';')
            .and_then(|(semi, _)| decode_entity(&after[..semi]).map(|ch| (ch, semi)));
        match decoded {
            Some((ch, semi)) => {
                if let Some(ch) = ch {
                    out.push(ch);
                }
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `Some(Some(c))` = entity decoded to `c`, `Some(None)` = entity decoded to
/// nothing (soft hyphen), `None` = not an entity we know.
fn decode_entity(name: &str) -> Option<Option<char>> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse::<u32>().ok()?,
        };
        return Some(Some(match code {
            0xA0 => ' ',
            0xAD => return Some(None),
            _ => char::from_u32(code).filter(|c| !c.is_control() || c.is_whitespace())?,
        }));
    }
    let c = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "shy" => return Some(None),
        "ndash" => '\u{2013}',
        "mdash" => '\u{2014}',
        "hellip" => '\u{2026}',
        "laquo" => '\u{ab}',
        "raquo" => '\u{bb}',
        "bdquo" => '\u{201e}',
        "ldquo" => '\u{201c}',
        "rdquo" => '\u{201d}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "copy" => '\u{a9}',
        _ => return None,
    };
    Some(Some(c))
}

/// Cut to at most `max` chars, preferably at a word boundary, with an ellipsis.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    let trimmed = match cut.rfind(' ') {
        Some(i) if cut[i..].chars().count() <= 40 => &cut[..i],
        _ => cut.as_str(),
    };
    format!("{}\u{2026}", trimmed.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clean_text_strips_tags_and_decodes_entities() {
        let html = "<p>Ala &amp; kot&nbsp;<strong data-x=\"1\">bez</strong> b\u{142}\u{119}d\u{f3}w &#8211; ok &#x2014; &hellip;</p>\n<p>drugi</p>";
        assert_eq!(
            clean_text(html),
            "Ala & kot bez b\u{142}\u{119}d\u{f3}w \u{2013} ok \u{2014} \u{2026} drugi"
        );
    }

    #[test]
    fn clean_text_drops_script_and_style_and_keeps_unknown_entities() {
        let html = "a<script>alert(1)</script>b<STYLE>p{}</STYLE>c &foo; & d";
        assert_eq!(clean_text(html), "abc &foo; & d");
    }

    #[test]
    fn truncate_adds_ellipsis_at_word_boundary() {
        let long = "slowo ".repeat(100);
        let out = truncate_chars(long.trim(), 300);
        assert!(out.chars().count() <= 300);
        assert!(out.ends_with("slowo\u{2026}"));
        assert_eq!(truncate_chars("krotko", 300), "krotko");
    }

    #[test]
    fn safe_https_rejects_other_schemes() {
        assert_eq!(safe_https(" https://igtsf.com/x/ "), Some("https://igtsf.com/x/".into()));
        assert_eq!(safe_https("javascript:alert(1)"), None);
        assert_eq!(safe_https("http://igtsf.com"), None);
        assert_eq!(safe_https("https://a b"), None);
    }

    #[test]
    fn maps_event_with_venue_and_image() {
        let body = json!({ "events": [{
            "id": 9911,
            "title": "Projekty &#8211; przyroda",
            "start_date": "2026-10-06 09:00:00",
            "end_date": "2026-10-07 15:00:00",
            "all_day": false,
            "url": "https://igtsf.com/wydarzenie/x/",
            "image": { "url": "https://igtsf.com/img.png" },
            "venue": { "venue": "Centrum", "city": "Krak\u{f3}w", "address": "ul. 1" },
            "description": "<p>Opis</p>"
        }]});
        let events = parse_events(&body);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.id, 9911);
        assert_eq!(e.title, "Projekty \u{2013} przyroda");
        assert_eq!(e.start, "2026-10-06T09:00:00");
        assert_eq!(e.end.as_deref(), Some("2026-10-07T15:00:00"));
        assert_eq!(e.venue.as_deref(), Some("Centrum"));
        assert_eq!(e.city.as_deref(), Some("Krak\u{f3}w"));
        assert_eq!(e.image.as_deref(), Some("https://igtsf.com/img.png"));
        assert_eq!(e.description, "Opis");
    }

    #[test]
    fn tolerates_online_event_shapes_and_skips_bad_entries() {
        let body = json!({ "events": [
            {
                "id": 1, "title": "Webinar", "start_date": "2026-10-06 09:00:00",
                "all_day": true, "venue": [], "image": false, "url": "javascript:x"
            },
            { "id": 2, "title": "Bez daty" },
            { "id": 3, "title": "Ukryte", "start_date": "2026-10-06 09:00:00",
              "hide_from_listings": true },
            { "title": "Bez id", "start_date": "2026-10-06 09:00:00" }
        ]});
        let events = parse_events(&body);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.id, 1);
        assert!(e.all_day);
        assert_eq!(e.venue, None);
        assert_eq!(e.image, None);
        assert_eq!(e.url, None);
        assert_eq!(e.end, None);
        assert_eq!(e.description, "");
    }

    #[test]
    fn non_object_body_gives_empty_list() {
        assert!(parse_events(&json!({ "code": "rest_no_route" })).is_empty());
        assert!(parse_events(&json!([])).is_empty());
    }
}
