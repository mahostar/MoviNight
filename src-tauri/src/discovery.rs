use crate::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Filters {
    pub content_type: String,
    pub year_from: Option<u32>,
    pub year_to: Option<u32>,
    pub genre_ids: Vec<u32>,
    pub sort_by: String,
    pub exclude_animation: bool,
    pub hide_incomplete: bool,
    pub with_watch_providers: Vec<u32>,
    pub with_original_language: Option<String>,
    pub min_rating: Option<f64>,
    pub watch_region: String,
}
impl Default for Filters {
    fn default() -> Self {
        Self {
            content_type: "all".into(),
            year_from: None,
            year_to: None,
            genre_ids: vec![],
            sort_by: "popularity.desc".into(),
            exclude_animation: false,
            hide_incomplete: false,
            with_watch_providers: vec![],
            with_original_language: None,
            min_rating: None,
            watch_region: "US".into(),
        }
    }
}
impl Filters {
    fn validate(&self) -> Result<(), ApiError> {
        if self.hide_incomplete && self.min_rating.is_some_and(|r| r >= 10.0) {
            return Err(ApiError::Invalid(
                "Hide incomplete excludes ratings of exactly 10. Choose a minimum rating below 10."
                    .into(),
            ));
        }
        if !["all", "movie", "tv"].contains(&self.content_type.as_str()) {
            return Err(ApiError::Invalid(
                "Content type must be all, movie or tv".into(),
            ));
        }
        if self.watch_region.len() != 2
            || !self.watch_region.bytes().all(|b| b.is_ascii_uppercase())
        {
            return Err(ApiError::Invalid(
                "Use a two-letter uppercase watch region".into(),
            ));
        }
        if self.genre_ids.len() > 100
            || self.with_watch_providers.len() > 100
            || self
                .genre_ids
                .iter()
                .chain(&self.with_watch_providers)
                .any(|id| *id == 0)
        {
            return Err(ApiError::Invalid(
                "Use at most 100 positive genre/provider IDs".into(),
            ));
        }
        api::discover_params(
            "movie",
            1,
            self.year_from,
            self.year_to,
            self.genre_ids.clone(),
            self.sort_by.clone(),
            self.exclude_animation,
            self.with_watch_providers.clone(),
            self.with_original_language.clone(),
            self.min_rating,
        )?;
        Ok(())
    }
}

pub async fn options(state: &AppState, region: &str) -> Result<Value, ApiError> {
    Filters {
        watch_region: region.into(),
        ..Default::default()
    }
    .validate()?;
    let mut out = json!({"watch_region":region,"sort_orders":["popularity.desc","popularity.asc","vote_average.desc","vote_average.asc","release_date.desc","release_date.asc"]});
    out["languages"] = state
        .http
        .get(state, "configuration/languages", &[], false)
        .await?;
    for kind in ["movie", "tv"] {
        let genres: Value = state
            .http
            .get(state, &format!("genre/{kind}/list"), &[], false)
            .await?;
        let providers: Value = state
            .http
            .get(
                state,
                &format!("watch/providers/{kind}"),
                &[("watch_region".into(), region.into())],
                false,
            )
            .await?;
        out[kind] = json!({"genres":genres["genres"],"providers":providers["results"].as_array().into_iter().flatten().map(|p| json!({"id":p["provider_id"],"name":p["provider_name"]})).collect::<Vec<_>>()});
    }
    Ok(out)
}

async fn kind_page(
    state: &AppState,
    filters: &Filters,
    kind: &str,
    page: u32,
) -> Result<Value, ApiError> {
    if filters.content_type != "all" && filters.content_type != kind {
        return Ok(json!({"results":[],"total_pages":0,"total_results":0}));
    }
    // Movie and TV genre IDs differ. Only apply selected genres valid for this format.
    let genres = if filters.genre_ids.is_empty() {
        vec![]
    } else {
        let reference: GenreResponse = state
            .http
            .get(state, &format!("genre/{kind}/list"), &[], false)
            .await?;
        filters
            .genre_ids
            .iter()
            .copied()
            .filter(|id| reference.genres.iter().any(|g| g.id == *id))
            .collect::<Vec<_>>()
    };
    if !filters.genre_ids.is_empty() && genres.is_empty() {
        return Ok(json!({"results":[],"total_pages":0,"total_results":0}));
    }
    let mut params = api::discover_params(
        kind,
        page,
        filters.year_from,
        filters.year_to,
        genres,
        filters.sort_by.clone(),
        filters.exclude_animation,
        filters.with_watch_providers.clone(),
        filters.with_original_language.clone(),
        filters.min_rating,
    )?;
    if filters.hide_incomplete {
        // Apply rating exclusions BEFORE TMDB sorts and paginates, so highest/lowest
        // rated searches do not fill every page with titles the UI must discard.
        let lower = filters.min_rating.unwrap_or(0.0).max(0.000001).to_string();
        if let Some((_, value)) = params.iter_mut().find(|(key, _)| key == "vote_average.gte") {
            *value = lower;
        } else {
            params.push(("vote_average.gte".into(), lower));
        }
        params.push(("vote_average.lte".into(), "9.999999".into()));
    }
    for (key, value) in &mut params {
        if key == "watch_region" {
            *value = filters.watch_region.clone();
        }
    }
    let mut response: Value = state
        .http
        .get(state, &format!("discover/{kind}"), &params, false)
        .await?;
    if let Some(items) = response["results"].as_array_mut() {
        for item in items {
            item["content_type"] = json!(kind);
        }
    }
    Ok(response)
}
pub async fn page(state: &AppState, filters: &Filters, page: u32) -> Result<Value, ApiError> {
    filters.validate()?;
    if !(1..=500).contains(&page) {
        return Err(ApiError::Invalid("Page must be 1–500".into()));
    }
    let (movies, tv) = tokio::try_join!(
        kind_page(state, filters, "movie", page),
        kind_page(state, filters, "tv", page)
    )?;
    let mut results = vec![];
    // Interleave independently ranked streams, keeping both formats represented.
    let m = movies["results"].as_array().cloned().unwrap_or_default();
    let t = tv["results"].as_array().cloned().unwrap_or_default();
    for i in 0..m.len().max(t.len()) {
        if let Some(item) = m.get(i) {
            results.push(item.clone());
        }
        if let Some(item) = t.get(i) {
            results.push(item.clone());
        }
    }
    let raw_count = results.len();
    // TMDB can round a borderline score to 10 in its response even with an upper
    // query bound. Enforce the exact rule, including poster presence, on returned data.
    results.retain(|item| eligible(item, filters));
    let filtered_out = raw_count - results.len();
    Ok(json!({"page":page,"results":results,
        "filtered_out":filtered_out,
        "total_pages":movies["total_pages"].as_u64().unwrap_or(0).max(tv["total_pages"].as_u64().unwrap_or(0)).min(500),
        "total_results":movies["total_results"].as_u64().unwrap_or(0)+tv["total_results"].as_u64().unwrap_or(0)}))
}
pub fn eligible(item: &Value, filters: &Filters) -> bool {
    !filters.hide_incomplete
        || (item["poster_path"]
            .as_str()
            .is_some_and(|p| !p.trim().is_empty())
            && item["vote_average"]
                .as_f64()
                .is_some_and(|r| r > 0.0 && r < 10.0))
}
// An explicit allowlist keeps images, videos, homepages and unrelated TMDB fields out of agent context.
pub fn compact(item: &Value, kind: &str) -> Value {
    let mut out = json!({"id":item["id"],"content_type":kind,"title":item.get("title").or_else(|| item.get("name")).unwrap_or(&Value::Null),
        "overview":item["overview"],"release_date":item.get("release_date").or_else(|| item.get("first_air_date")).unwrap_or(&Value::Null),"vote_average":item["vote_average"]});
    for key in [
        "original_title",
        "original_name",
        "original_language",
        "vote_count",
        "genre_ids",
        "genres",
        "runtime",
        "episode_run_time",
        "number_of_seasons",
        "number_of_episodes",
        "status",
        "watched_date",
        "white_list_date",
        "watched_seasons",
        "total_seasons_known",
        "has_new_seasons",
    ] {
        if let Some(value) = item.get(key) {
            out[key] = value.clone();
        }
    }
    if let Some(seasons) = item["seasons"].as_array() {
        out["seasons"]=json!(seasons.iter().map(|s| json!({"season_number":s["season_number"],"name":s["name"],"air_date":s["air_date"],"episode_count":s["episode_count"],"overview":s["overview"]})).collect::<Vec<_>>());
    }
    out
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    filters: Filters,
    page: u32,
    offset: usize,
}
fn encode(cursor: &Cursor) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(cursor).unwrap())
}
fn decode(value: &str) -> Result<Cursor, ApiError> {
    if value.len() > 16000 {
        return Err(ApiError::Invalid("Invalid discovery cursor".into()));
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ApiError::Invalid("Invalid discovery cursor".into()))?;
    let cursor: Cursor = serde_json::from_slice(&bytes)?;
    cursor.filters.validate()?;
    if !(1..=500).contains(&cursor.page) || cursor.offset > 40 {
        return Err(ApiError::Invalid(
            "Invalid discovery cursor position".into(),
        ));
    }
    Ok(cursor)
}
pub async fn absorb(
    state: &AppState,
    filters: Filters,
    cursor: Option<&str>,
    limit: u32,
) -> Result<Value, ApiError> {
    let (watched, waitlist) = {
        let _guard = state.data_lock.lock().unwrap();
        let dir = get_config_dir()?;
        let watched: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
        let waitlist: Vec<WhiteListItem> =
            storage::read(&dir, "white_list.json")?.unwrap_or_default();
        (watched, waitlist)
    };
    absorb_snapshot(state, filters, cursor, limit, &watched, &waitlist).await
}
async fn absorb_snapshot(
    state: &AppState,
    filters: Filters,
    cursor: Option<&str>,
    limit: u32,
    watched: &[WatchedItem],
    waitlist: &[WhiteListItem],
) -> Result<Value, ApiError> {
    if !(1..=1000).contains(&limit) {
        return Err(ApiError::Invalid("Choose 1–1000 titles per batch".into()));
    }
    let mut position = if let Some(cursor) = cursor {
        decode(cursor)?
    } else {
        filters.validate()?;
        Cursor {
            filters,
            page: 1,
            offset: 0,
        }
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(50);
    let mut titles = vec![];
    let mut scanned = 0;
    let mut exhausted = false;
    let mut warning = None;
    // Bound work for sparse filters; preserve a retryable cursor on timeouts/rate limits.
    for _ in 0..100 {
        let response =
            match tokio::time::timeout_at(deadline, page(state, &position.filters, position.page))
                .await
            {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => {
                    if titles.is_empty() {
                        return Err(error);
                    }
                    warning = Some(error.to_string());
                    break;
                }
                Err(_) => {
                    warning = Some("Time budget reached; continue with next_cursor".into());
                    break;
                }
            };
        let items = response["results"].as_array().unwrap();
        if position.offset == 0 {
            scanned += response["filtered_out"].as_u64().unwrap_or(0) as usize;
        }
        while position.offset < items.len() && titles.len() < limit as usize {
            let item = &items[position.offset];
            position.offset += 1;
            scanned += 1;
            if !eligible(item, &position.filters) {
                continue;
            }
            let kind = item["content_type"].as_str().unwrap();
            let mut title = compact(item, kind);
            let w = watched
                .iter()
                .find(|i| item["id"] == i.id && kind == i.content_type);
            title["library_status"] = json!(if w.is_some() {
                "watched"
            } else if waitlist
                .iter()
                .any(|i| item["id"] == i.id && kind == i.content_type)
            {
                "waitlist"
            } else {
                "unsaved"
            });
            if let Some(w) = w {
                title["watched_date"] = json!(w.watched_date);
                title["watched_seasons"] = json!(w.watched_seasons);
            }
            titles.push(title);
        }
        if position.offset >= items.len() {
            if position.page >= response["total_pages"].as_u64().unwrap_or(0) as u32 {
                exhausted = true;
                break;
            }
            position.page += 1;
            position.offset = 0;
        }
        if titles.len() >= limit as usize {
            break;
        }
    }
    Ok(
        json!({"titles":titles,"returned":titles.len(),"scanned":scanned,"requested":limit,"exhausted":exhausted,
        "next_cursor":if exhausted { None } else { Some(encode(&position)) },"warning":warning,
        "filters":position.filters,"ordering":"Movies and TV are independently ranked and interleaved; not a global ranking.",
        "next_action":"Use absorb_next_titles with next_cursor to research another large batch. Publish your selected recommendations with publish_suggestion(source=discovery); they appear in Suggestions for user approval."}),
    )
}
#[tauri::command]
pub async fn discover_titles_page(
    filters: Filters,
    page_number: u32,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Value, ApiError> {
    page(&state, &filters, page_number).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn absorbs_thousands_and_resumes_partial_pages_without_skips() {
        use axum::{
            extract::{Path, Query},
            routing::get,
            Json, Router,
        };
        use std::collections::{HashMap, HashSet};
        let router = Router::new().route("/genre/:kind/list",get(|Path(kind): Path<String>| async move {
            Json(json!({"genres":[{"id":if kind=="movie" {28} else {10759},"name":"Action"}]}))
        })).route("/discover/:kind", get(|Path(kind): Path<String>, Query(params): Query<HashMap<String,String>>| async move {
            assert_eq!(params["with_genres"],if kind=="movie" {"28"} else {"10759"});
            assert_eq!(params["with_original_language"], "en");
            assert_eq!(params["with_watch_providers"], "8|9");
            assert_eq!(params["watch_region"], "GB");
            assert_eq!(params["vote_average.gte"], "7");
            assert!(params["vote_average.lte"].parse::<f64>().unwrap() < 10.0);
            assert_eq!(params["without_genres"], "16");
            let date=if kind=="movie" {"primary_release_date"} else {"first_air_date"};
            assert_eq!(params[&format!("{date}.gte")], "2000-01-01");
            assert_eq!(params[&format!("{date}.lte")], "2025-12-31");
            assert_eq!(params["sort_by"], format!("{date}.desc"));
            let page=params["page"].parse::<u32>().unwrap();
            // Include missing artwork and a score rounded to 10 despite the query bound.
            let results=(0..20).map(|i| json!({"id":(page-1)*20+i+1,"title":format!("{kind} {page} {i}"),"overview":"Useful plot","poster_path":if i==0 {None} else {Some("/private.jpg")},"vote_average":if i==1 {10} else {8},"vote_count":100,"original_language":"en","youtube_url":"should not reach the agent","release_date":"2020-01-01"})).collect::<Vec<_>>();
            Json(json!({"results":results,"total_pages":60,"total_results":1200,"page":page}))
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let state = AppState {
            api_key: Mutex::new(Some("test-key".into())),
            data_lock: Mutex::new(()),
            http: TmdbClient::for_test(format!("http://{address}")),
            mcp: Mutex::new(ServerControl::default()),
        };
        let filters = Filters {
            genre_ids: vec![28, 10759],
            year_from: Some(2000),
            year_to: Some(2025),
            with_original_language: Some("en".into()),
            with_watch_providers: vec![9, 8],
            watch_region: "GB".into(),
            min_rating: Some(7.0),
            exclude_animation: true,
            hide_incomplete: true,
            sort_by: "release_date.desc".into(),
            ..Default::default()
        };
        let mut out = absorb_snapshot(&state, filters.clone(), None, 1000, &[], &[])
            .await
            .unwrap();
        let mut seen = HashSet::new();
        assert!(out["scanned"].as_u64().unwrap() > 1000);
        for expected in [1000, 1000, 17, 143] {
            assert_eq!(out["returned"], expected);
            for item in out["titles"].as_array().unwrap() {
                assert!(seen.insert((
                    item["content_type"].as_str().unwrap().to_string(),
                    item["id"].as_u64().unwrap()
                )));
                assert!(item.get("poster_path").is_none() && item.get("youtube_url").is_none());
                assert!(item["vote_average"].as_f64().unwrap() < 10.0);
            }
            if let Some(cursor) = out["next_cursor"].as_str() {
                let next = if expected == 1000 && seen.len() == 2000 {
                    17
                } else if expected == 17 {
                    1000
                } else {
                    1000
                };
                out = absorb_snapshot(&state, Default::default(), Some(cursor), next, &[], &[])
                    .await
                    .unwrap();
            }
        }
        assert_eq!(seen.len(), 2160);
        assert_eq!(out["exhausted"], true);
        assert!(out["next_cursor"].is_null());
        assert!(
            absorb_snapshot(&state, filters.clone(), None, 1001, &[], &[])
                .await
                .is_err()
        );
        assert!(decode("bad cursor").is_err());
        server.abort();
    }
    #[test]
    fn validates_filters_and_removes_visual_fields_from_details() {
        assert!(Filters {
            hide_incomplete: true,
            min_rating: Some(10.0),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(Filters {
            year_from: Some(2025),
            year_to: Some(2020),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(Filters {
            content_type: "both".into(),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(Filters {
            min_rating: Some(11.0),
            ..Default::default()
        }
        .validate()
        .is_err());
        let item = json!({"id":1,"name":"Series","overview":"Plot","poster_path":"/art.jpg","homepage":"https://example.org","seasons":[{"season_number":2,"overview":"Season plot","air_date":"2020-01-01","poster_path":"/season.jpg"}],"watched_seasons":[1]});
        let out = compact(&item, "tv");
        assert_eq!(out["title"], "Series");
        assert_eq!(out["watched_seasons"], json!([1]));
        assert!(out.get("poster_path").is_none() && out.get("homepage").is_none());
        assert!(out["seasons"][0].get("poster_path").is_none());
    }
}
