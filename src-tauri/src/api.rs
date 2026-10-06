use crate::*;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

struct Entry {
    value: Value,
    expires: Instant,
}
pub fn disk_key(path: &str, params: &[(String, String)]) -> String {
    let mut params = params.to_vec();
    params.sort();
    format!(
        "tmdb:{path}:en-US:{}",
        serde_json::to_string(&params).unwrap()
    )
}
pub struct TmdbClient {
    client: reqwest::Client,
    pub image_client: reqwest::Client,
    pub offline: crate::offline::OfflineStore,
    base_url: String,
    cache: Mutex<HashMap<String, Entry>>,
    flights: Mutex<HashMap<String, std::sync::Weak<tokio::sync::Mutex<()>>>>,
}
impl TmdbClient {
    pub fn new() -> Self {
        Self {
            base_url: "https://api.themoviedb.org/3".into(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .unwrap(),
            image_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(12))
                .build()
                .unwrap(),
            offline: crate::offline::OfflineStore::new(),
            cache: Mutex::new(HashMap::new()),
            flights: Mutex::new(HashMap::new()),
        }
    }
    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }
    fn cached(&self, key: &str) -> Option<Value> {
        let cache = self.cache.lock().unwrap();
        cache
            .get(key)
            .filter(|e| e.expires > Instant::now())
            .map(|e| e.value.clone())
    }
    pub async fn get<T: DeserializeOwned>(
        &self,
        state: &AppState,
        path: &str,
        params: &[(String, String)],
        refresh: bool,
    ) -> Result<T, ApiError> {
        let api_key = state
            .api_key
            .lock()
            .unwrap()
            .clone()
            .ok_or(ApiError::NoApiKey)?;
        let mut url = reqwest::Url::parse(&format!("{}/{path}", self.base_url)).unwrap();
        url.query_pairs_mut()
            .append_pair("api_key", &api_key)
            .append_pair("language", "en-US")
            .extend_pairs(params.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        let key = url.to_string(); // Memory only; credentials are never written to cache files or logs.
        if !refresh {
            if let Some(v) = self.cached(&key) {
                return Ok(serde_json::from_value(v)?);
            }
        }
        let flight = {
            let mut flights = self.flights.lock().unwrap();
            flights.retain(|_, v| v.strong_count() > 0);
            if let Some(lock) = flights.get(&key).and_then(|v| v.upgrade()) {
                lock
            } else {
                let lock = Arc::new(tokio::sync::Mutex::new(()));
                flights.insert(key.clone(), Arc::downgrade(&lock));
                lock
            }
        };
        let _guard = flight.lock().await;
        if !refresh {
            if let Some(v) = self.cached(&key) {
                return Ok(serde_json::from_value(v)?);
            }
        }
        let disk_key = disk_key(path, params);
        let epoch = self.offline.epoch.load(std::sync::atomic::Ordering::SeqCst);
        let fetched = async {
            self.client
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .json::<Value>()
                .await
        }
        .await;
        let value = match fetched {
            Ok(value) => {
                self.offline
                    .offline
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                value
            }
            Err(error) => {
                // Authentication, validation and rate-limit failures must never be hidden by old data.
                let unavailable = error.is_connect()
                    || error.is_timeout()
                    || error.status().is_some_and(|s| s.is_server_error());
                if unavailable {
                    self.offline
                        .offline
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    if let Some(value) = self.offline.load_json(&disk_key) {
                        return Ok(serde_json::from_value(value)?);
                    }
                    return Err(ApiError::Invalid("Offline: this page or title has not been downloaded. Reconnect to fetch it, or return to previously browsed filters.".into()));
                }
                return Err(error.into());
            }
        };
        let parsed = serde_json::from_value(value.clone())?;
        if epoch == self.offline.epoch.load(std::sync::atomic::Ordering::SeqCst) {
            self.offline.save_json(
                &disk_key,
                &value,
                if path.starts_with("discover/") { 0 } else { 2 },
            );
        }
        let ttl = if path.starts_with("genre/")
            || path.starts_with("configuration/")
            || path.starts_with("watch/providers/")
        {
            86400
        } else if path.starts_with("discover/") || path.starts_with("search/") {
            300
        } else {
            21600
        };
        let mut cache = self.cache.lock().unwrap();
        cache.retain(|_, e| e.expires > Instant::now());
        if cache.len() >= 256 {
            if let Some(oldest) = cache
                .iter()
                .min_by_key(|(_, e)| e.expires)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest);
            }
        }
        cache.insert(
            key,
            Entry {
                value,
                expires: Instant::now() + Duration::from_secs(ttl),
            },
        );
        Ok(parsed)
    }
}
#[tauri::command]
pub fn clear_api_cache(state: State<'_, std::sync::Arc<AppState>>) {
    state.http.clear();
}
#[tauri::command]
pub async fn get_movie_genres(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<Genre>, ApiError> {
    Ok(state
        .http
        .get::<GenreResponse>(&state, "genre/movie/list", &[], false)
        .await?
        .genres)
}
#[tauri::command]
pub async fn get_tv_genres(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<Genre>, ApiError> {
    Ok(state
        .http
        .get::<GenreResponse>(&state, "genre/tv/list", &[], false)
        .await?
        .genres)
}
#[tauri::command]
pub async fn get_languages(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<Language>, ApiError> {
    state
        .http
        .get(&state, "configuration/languages", &[], false)
        .await
}
#[tauri::command]
pub async fn get_watch_providers(
    content_type: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<WatchProvider>, ApiError> {
    validate_type(&content_type)?;
    Ok(state
        .http
        .get::<WatchProviderResponse>(
            &state,
            &format!("watch/providers/{content_type}"),
            &[("watch_region".into(), "US".into())],
            false,
        )
        .await?
        .results)
}
fn discover_params(
    kind: &str,
    page: u32,
    year_from: Option<u32>,
    year_to: Option<u32>,
    mut genre_ids: Vec<u32>,
    sort_by: String,
    exclude_animation: bool,
    mut providers: Vec<u32>,
    language: Option<String>,
    rating: Option<f64>,
) -> Result<Vec<(String, String)>, ApiError> {
    if !(1..=500).contains(&page) {
        return Err(ApiError::Invalid("Page must be between 1 and 500".into()));
    }
    if matches!((year_from,year_to),(Some(a),Some(b)) if a>b) {
        return Err(ApiError::Invalid("Start year must precede end year".into()));
    }
    let mut p = vec![
        ("page".into(), page.to_string()),
        ("include_adult".into(), "false".into()),
        (
            "sort_by".into(),
            if kind == "movie" {
                sort_by.replace("release_date.", "primary_release_date.")
            } else {
                sort_by.replace("release_date.", "first_air_date.")
            },
        ),
    ];
    let date = if kind == "movie" {
        "primary_release_date"
    } else {
        "first_air_date"
    };
    if let Some(y) = year_from {
        p.push((format!("{date}.gte"), format!("{y}-01-01")));
    }
    if let Some(y) = year_to {
        p.push((format!("{date}.lte"), format!("{y}-12-31")));
    }
    genre_ids.sort_unstable();
    genre_ids.dedup();
    if !genre_ids.is_empty() {
        p.push((
            "with_genres".into(),
            genre_ids
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join("|"),
        ));
    }
    if exclude_animation {
        p.push(("without_genres".into(), "16".into()));
    }
    providers.sort_unstable();
    providers.dedup();
    if !providers.is_empty() {
        p.push((
            "with_watch_providers".into(),
            providers
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join("|"),
        ));
        p.push(("watch_region".into(), "US".into()));
    }
    if let Some(l) = language.filter(|l| !l.is_empty()) {
        p.push(("with_original_language".into(), l));
    }
    if let Some(r) = rating {
        if !(0.0..=10.0).contains(&r) {
            return Err(ApiError::Invalid("Rating must be 0–10".into()));
        }
        if r > 0.0 {
            p.push(("vote_average.gte".into(), r.to_string()));
        }
    }
    Ok(p)
}
#[tauri::command]
pub async fn search_movies(
    query: String,
    page: u32,
    year_from: Option<u32>,
    year_to: Option<u32>,
    genre_ids: Vec<u32>,
    sort_by: String,
    exclude_animation: bool,
    with_watch_providers: Vec<u32>,
    with_original_language: Option<String>,
    min_rating: Option<f64>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<ApiResponse<Movie>, ApiError> {
    let p = if query.trim().is_empty() {
        discover_params(
            "movie",
            page,
            year_from,
            year_to,
            genre_ids,
            sort_by,
            exclude_animation,
            with_watch_providers,
            with_original_language,
            min_rating,
        )?
    } else {
        search_params(&query, page)?
    };
    state
        .http
        .get(
            &state,
            if query.trim().is_empty() {
                "discover/movie"
            } else {
                "search/movie"
            },
            &p,
            false,
        )
        .await
}
#[tauri::command]
pub async fn search_tv_shows(
    query: String,
    page: u32,
    year_from: Option<u32>,
    year_to: Option<u32>,
    genre_ids: Vec<u32>,
    sort_by: String,
    exclude_animation: bool,
    with_watch_providers: Vec<u32>,
    with_original_language: Option<String>,
    min_rating: Option<f64>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<ApiResponse<TvShow>, ApiError> {
    let p = if query.trim().is_empty() {
        discover_params(
            "tv",
            page,
            year_from,
            year_to,
            genre_ids,
            sort_by,
            exclude_animation,
            with_watch_providers,
            with_original_language,
            min_rating,
        )?
    } else {
        search_params(&query, page)?
    };
    state
        .http
        .get(
            &state,
            if query.trim().is_empty() {
                "discover/tv"
            } else {
                "search/tv"
            },
            &p,
            false,
        )
        .await
}
pub fn search_params(query: &str, page: u32) -> Result<Vec<(String, String)>, ApiError> {
    if query.trim().is_empty() || query.len() > 500 {
        return Err(ApiError::Invalid(
            "Enter a title up to 500 characters".into(),
        ));
    }
    if !(1..=500).contains(&page) {
        return Err(ApiError::Invalid("Page must be 1–500".into()));
    }
    Ok(vec![
        ("query".into(), query.trim().into()),
        ("page".into(), page.to_string()),
        ("include_adult".into(), "false".into()),
    ])
}
#[tauri::command]
pub async fn get_movie_details(
    id: u32,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<MovieDetails, ApiError> {
    state
        .http
        .get(&state, &format!("movie/{id}"), &[], false)
        .await
}
#[tauri::command]
pub async fn get_tv_details(
    id: u32,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<TvDetails, ApiError> {
    state
        .http
        .get(&state, &format!("tv/{id}"), &[], false)
        .await
}
#[tauri::command]
pub async fn get_trailers(
    id: u32,
    content_type: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<Trailer>, ApiError> {
    validate_type(&content_type)?;
    let response: TrailerResponse = state
        .http
        .get(&state, &format!("{content_type}/{id}/videos"), &[], false)
        .await?;
    let mut trailers: Vec<_> = response
        .results
        .into_iter()
        .filter(|t| {
            t.site == "YouTube" && (t.trailer_type == "Trailer" || t.trailer_type == "Teaser")
        })
        .collect();
    trailers.sort_by(|a, b| {
        b.official
            .cmp(&a.official)
            .then(b.published_at.cmp(&a.published_at))
    });
    Ok(trailers)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn successful_requests_coalesce_and_errors_are_not_cached() {
        use axum::{http::StatusCode, routing::get, Json, Router};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = requests.clone();
        let router = Router::new()
            .route(
                "/search/movie",
                get(move || {
                    let count = counter.clone();
                    async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(30)).await;
                        Json(serde_json::json!({"results":[]}))
                    }
                }),
            )
            .route("/failure", get(|| async { StatusCode::TOO_MANY_REQUESTS }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let mut client = TmdbClient::new();
        client.base_url = format!("http://{address}");
        let state = AppState {
            api_key: Mutex::new(Some("test-key".into())),
            data_lock: Mutex::new(()),
            http: client,
            mcp: Mutex::new(ServerControl::default()),
        };
        let (a, b) = tokio::join!(
            state.http.get::<Value>(&state, "search/movie", &[], false),
            state.http.get::<Value>(&state, "search/movie", &[], false)
        );
        assert!(a.is_ok() && b.is_ok());
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        state
            .http
            .get::<Value>(&state, "search/movie", &[], false)
            .await
            .unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        state.http.clear();
        state
            .http
            .get::<Value>(&state, "search/movie", &[], false)
            .await
            .unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert!(state
            .http
            .get::<Value>(&state, "failure", &[], false)
            .await
            .is_err());
        assert_eq!(state.http.cache.lock().unwrap().len(), 1);
        server.abort();
    }
    #[tokio::test]
    async fn persistent_fallback_does_not_cover_fresh_online_data_or_auth_errors() {
        use axum::{http::StatusCode, response::IntoResponse, routing::get, Json, Router};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mode = Arc::new(AtomicUsize::new(1));
        let handler = mode.clone();
        let router=Router::new().route("/discover/movie",get(move || {
            let mode=handler.clone();
            async move {
                let mode=mode.load(Ordering::SeqCst);
                if mode==401{return StatusCode::UNAUTHORIZED.into_response()}
                Json(serde_json::json!({"version":mode,"results":[],"homepage":"https://example.org","extra":{"preserved":true}})).into_response()
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let mut client = TmdbClient::new();
        client.base_url = format!("http://{address}");
        let mut state = AppState {
            api_key: Mutex::new(Some("private-test-key".into())),
            data_lock: Mutex::new(()),
            http: client,
            mcp: Mutex::new(ServerControl::default()),
        };
        let first: Value = state
            .http
            .get(&state, "discover/movie", &[], true)
            .await
            .unwrap();
        assert_eq!(first["version"], 1);
        mode.store(2, Ordering::SeqCst);
        let online: Value = state
            .http
            .get(&state, "discover/movie", &[], true)
            .await
            .unwrap();
        assert_eq!(online["version"], 2);
        mode.store(401, Ordering::SeqCst);
        assert!(state
            .http
            .get::<Value>(&state, "discover/movie", &[], true)
            .await
            .is_err());
        server.abort();
        let _ = server.await;
        state.http.clear();
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        state.http.base_url = format!("http://{}", closed.local_addr().unwrap());
        drop(closed);
        let offline: Value = state
            .http
            .get(&state, "discover/movie", &[], true)
            .await
            .unwrap();
        assert_eq!(offline, online);
        assert!(state.http.offline.offline.load(Ordering::Relaxed));
        assert!(state
            .http
            .get::<Value>(&state, "search/movie", &[], true)
            .await
            .is_err());
        assert!(!disk_key("discover/movie", &[]).contains("private-test-key"));
    }
    #[test]
    fn genres_are_union_and_tv_dates_use_first_air_date() {
        let p = discover_params(
            "tv",
            1,
            Some(1990),
            Some(2026),
            vec![10765, 10759, 10765],
            "release_date.desc".into(),
            false,
            vec![],
            None,
            None,
        )
        .unwrap();
        assert!(p.contains(&("with_genres".into(), "10759|10765".into())));
        assert!(p.contains(&("first_air_date.gte".into(), "1990-01-01".into())));
        assert!(p.contains(&("sort_by".into(), "first_air_date.desc".into())));
    }
    #[test]
    fn movie_sort_uses_supported_primary_date() {
        let p = discover_params(
            "movie",
            1,
            None,
            None,
            vec![],
            "release_date.asc".into(),
            false,
            vec![],
            None,
            None,
        )
        .unwrap();
        assert!(p.contains(&("sort_by".into(), "primary_release_date.asc".into())));
    }
    #[test]
    fn expired_cache_is_not_returned() {
        let c = TmdbClient::new();
        c.cache.lock().unwrap().insert(
            "x".into(),
            Entry {
                value: serde_json::json!({"old":true}),
                expires: Instant::now() - Duration::from_secs(1),
            },
        );
        assert!(c.cached("x").is_none());
        c.cache.lock().unwrap().insert(
            "x".into(),
            Entry {
                value: serde_json::json!({"fresh":true}),
                expires: Instant::now() + Duration::from_secs(10),
            },
        );
        assert!(c.cached("x").is_some());
        c.clear();
        assert!(c.cached("x").is_none());
    }
}
