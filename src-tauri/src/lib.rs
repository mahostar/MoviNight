use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};
use tauri::State;
mod ai;
mod api;
mod discovery;
mod mcp;
mod offline;
mod storage;
use ai::*;
use api::*;
use mcp::*;

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    results: Vec<T>,
    total_pages: u32,
    total_results: u32,
    page: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Movie {
    id: u32,
    title: String,
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    release_date: String,
    vote_average: f64,
    vote_count: u32,
    genre_ids: Vec<u32>,
    popularity: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct TvShow {
    id: u32,
    name: String,
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    first_air_date: String,
    vote_average: f64,
    vote_count: u32,
    genre_ids: Vec<u32>,
    popularity: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Genre {
    id: u32,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GenreResponse {
    genres: Vec<Genre>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Language {
    iso_639_1: String,
    english_name: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct WatchProvider {
    provider_id: u32,
    provider_name: String,
    logo_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WatchProviderResponse {
    results: Vec<WatchProvider>,
}

// Watched item structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct WatchedItem {
    id: u32,
    title: String,
    overview: String,
    poster_path: Option<String>,
    #[serde(default)]
    release_date: String, // For movies it's release_date, for TV shows it's first_air_date
    vote_average: f64,
    content_type: String, // "movie" or "tv"
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
    watched_date: String, // ISO date when marked as watched
    // Season tracking fields (Option for backward compatibility with existing data)
    #[serde(default)]
    watched_seasons: Option<Vec<u32>>, // Season numbers the user has checked off
    #[serde(default)]
    total_seasons_known: Option<u32>, // Total seasons known at last check
    #[serde(default)]
    has_new_seasons: Option<bool>, // Flag: are there unwatched new seasons?
}

// White list item structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct WhiteListItem {
    id: u32,
    title: String,
    overview: String,
    poster_path: Option<String>,
    #[serde(default)]
    release_date: String, // For movies it's release_date, for TV shows it's first_air_date
    vote_average: f64,
    content_type: String, // "movie" or "tv"
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
    white_list_date: String, // ISO date when marked as white listed
}

// Trailer structures for TMDB API
#[derive(Debug, Serialize, Deserialize)]
struct Trailer {
    id: String,
    key: String,
    name: String,
    site: String,
    #[serde(rename = "type")]
    trailer_type: String,
    official: bool,
    published_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TrailerResponse {
    results: Vec<Trailer>,
}

// Movie/TV details structures
#[derive(Debug, Serialize, Deserialize)]
struct MovieDetails {
    id: u32,
    title: String,
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    release_date: String,
    vote_average: f64,
    vote_count: u32,
    genres: Vec<Genre>,
    runtime: Option<u32>,
    tagline: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TvDetails {
    id: u32,
    name: String,
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    first_air_date: String,
    vote_average: f64,
    vote_count: u32,
    genres: Vec<Genre>,
    number_of_seasons: u32,
    number_of_episodes: u32,
    tagline: Option<String>,
}

// Season info from TMDB TV details response
#[derive(Debug, Serialize, Deserialize, Clone)]
struct SeasonInfo {
    season_number: u32,
    name: String,
    episode_count: u32,
    air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
}

// Full TV details response including seasons array
#[derive(Debug, Serialize, Deserialize)]
struct TvDetailsFull {
    id: u32,
    name: String,
    number_of_seasons: u32,
    number_of_episodes: u32,
    seasons: Vec<SeasonInfo>,
}

// Response from get_tv_season_details command
#[derive(Debug, Serialize, Deserialize)]
struct TvSeasonDetailsResponse {
    id: u32,
    name: String,
    number_of_seasons: u32,
    seasons: Vec<SeasonInfo>,  // All seasons (excluding season 0/specials)
    watched_seasons: Vec<u32>, // Currently checked-off seasons
    total_seasons_known: u32,  // Total seasons known from last check
}

// Response item for new season check
#[derive(Debug, Serialize, Deserialize)]
struct NewSeasonAlert {
    id: u32,
    title: String,
    new_season_count: u32, // How many new seasons detected
    total_seasons: u32,    // Current total seasons on TMDB
}

struct AppState {
    api_key: Mutex<Option<String>>,
    data_lock: Mutex<()>,
    http: api::TmdbClient,
    mcp: Mutex<mcp::ServerControl>,
}

#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("No API key found. Set your TMDB API key in Settings.")]
    NoApiKey,
    #[error("Offline cache error: {0}")]
    Cache(String),
    #[error("{0}")]
    Invalid(String),
    #[error("TMDB request failed: {0}")]
    RequestFailed(reqwest::Error),
    #[error("Saved data could not be read: {0}. The original file has been preserved.")]
    ParseError(#[from] serde_json::Error),
    #[error("File system error: {0}")]
    FileError(#[from] std::io::Error),
}
impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        Self::RequestFailed(error.without_url())
    }
}
impl serde::Serialize for ApiError {
    fn serialize<S: serde::ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
fn get_config_dir() -> Result<PathBuf, ApiError> {
    // Debug-only isolation for runtime QA; release builds always use the established location.
    #[cfg(debug_assertions)]
    if let Some(test_dir) = std::env::var_os("MOVINIGHT_QA_DATA_DIR") {
        let dir = PathBuf::from(test_dir);
        std::fs::create_dir_all(&dir)?;
        return Ok(dir);
    }

    let dir = dirs::config_dir()
        .ok_or_else(|| ApiError::Invalid("Config directory unavailable".into()))?
        .join("movinight");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
fn validate_type(value: &str) -> Result<(), ApiError> {
    if value != "movie" && value != "tv" {
        return Err(ApiError::Invalid("Content type must be movie or tv".into()));
    }
    Ok(())
}
#[tauri::command]
async fn save_api_key(
    api_key: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), ApiError> {
    if api_key.trim().is_empty() {
        return Err(ApiError::NoApiKey);
    }
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut config: serde_json::Value =
        storage::read(&dir, "config.json")?.unwrap_or(serde_json::json!({}));
    config["api_key"] = serde_json::json!(api_key.trim());
    storage::write(&dir, "config.json", &config)?;
    *state.api_key.lock().unwrap() = Some(api_key.trim().into());
    state.http.clear();
    Ok(())
}
#[tauri::command]
async fn load_api_key(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Option<String>, ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    let config: Option<serde_json::Value> = storage::read(&get_config_dir()?, "config.json")?;
    let key = config.and_then(|v| v["api_key"].as_str().map(str::to_owned));
    *state.api_key.lock().unwrap() = key.clone();
    Ok(key)
}
#[tauri::command]
async fn get_watched_items(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<WatchedItem>, ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    Ok(storage::read(&get_config_dir()?, "watched.json")?.unwrap_or_default())
}
#[tauri::command]
async fn get_white_list_items(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<WhiteListItem>, ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    Ok(storage::read(&get_config_dir()?, "white_list.json")?.unwrap_or_default())
}
#[tauri::command]
async fn add_watched_item(
    id: u32,
    title: String,
    overview: String,
    poster_path: Option<String>,
    release_date: String,
    vote_average: f64,
    content_type: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), ApiError> {
    validate_type(&content_type)?;
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut items: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
    // Idempotent additions preserve the original watched date and season history.
    if items
        .iter()
        .any(|i| i.id == id && i.content_type == content_type)
    {
        return Ok(());
    }
    items.push(WatchedItem {
        extra: Default::default(),
        id,
        title,
        overview,
        poster_path,
        release_date,
        vote_average,
        content_type,
        watched_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        watched_seasons: None,
        total_seasons_known: None,
        has_new_seasons: None,
    });
    storage::write(&dir, "watched.json", &items)
}
#[tauri::command]
async fn add_white_list_item(
    id: u32,
    title: String,
    overview: String,
    poster_path: Option<String>,
    release_date: String,
    vote_average: f64,
    content_type: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), ApiError> {
    validate_type(&content_type)?;
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut items: Vec<WhiteListItem> = storage::read(&dir, "white_list.json")?.unwrap_or_default();
    if items
        .iter()
        .any(|i| i.id == id && i.content_type == content_type)
    {
        return Ok(());
    }
    items.push(WhiteListItem {
        extra: Default::default(),
        id,
        title,
        overview,
        poster_path,
        release_date,
        vote_average,
        content_type,
        white_list_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
    });
    storage::write(&dir, "white_list.json", &items)
}
#[tauri::command]
async fn remove_watched_item(
    id: u32,
    content_type: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut items: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
    items.retain(|i| !(i.id == id && i.content_type == content_type));
    storage::write(&dir, "watched.json", &items)
}
#[tauri::command]
async fn remove_white_list_item(
    id: u32,
    content_type: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut items: Vec<WhiteListItem> = storage::read(&dir, "white_list.json")?.unwrap_or_default();
    items.retain(|i| !(i.id == id && i.content_type == content_type));
    storage::write(&dir, "white_list.json", &items)
}
#[tauri::command]
async fn is_watched(
    id: u32,
    content_type: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<bool, ApiError> {
    Ok(get_watched_items(state)
        .await?
        .iter()
        .any(|i| i.id == id && i.content_type == content_type))
}
#[tauri::command]
async fn is_white_listed(
    id: u32,
    content_type: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<bool, ApiError> {
    Ok(get_white_list_items(state)
        .await?
        .iter()
        .any(|i| i.id == id && i.content_type == content_type))
}
#[tauri::command]
async fn get_tv_season_details(
    id: u32,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<TvSeasonDetailsResponse, ApiError> {
    let details: TvDetailsFull = state
        .http
        .get(&state, &format!("tv/{id}"), &[], false)
        .await?;
    let items = get_watched_items(state).await?;
    let item = items.iter().find(|i| i.id == id && i.content_type == "tv");
    Ok(TvSeasonDetailsResponse {
        id: details.id,
        name: details.name,
        number_of_seasons: details.number_of_seasons,
        seasons: details
            .seasons
            .into_iter()
            .filter(|s| s.season_number > 0)
            .collect(),
        watched_seasons: item
            .and_then(|i| i.watched_seasons.clone())
            .unwrap_or_default(),
        total_seasons_known: item.and_then(|i| i.total_seasons_known).unwrap_or(0),
    })
}
#[tauri::command]
async fn update_watched_seasons(
    id: u32,
    watched_seasons: Vec<u32>,
    total_seasons_known: u32,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut items: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
    let item = items
        .iter_mut()
        .find(|i| i.id == id && i.content_type == "tv")
        .ok_or_else(|| ApiError::Invalid("Show is no longer in your watched list".into()))?;
    item.has_new_seasons = Some((1..=total_seasons_known).any(|s| !watched_seasons.contains(&s)));
    item.watched_seasons = Some(watched_seasons);
    item.total_seasons_known = Some(total_seasons_known);
    storage::write(&dir, "watched.json", &items)
}
#[tauri::command]
async fn check_all_new_seasons(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<NewSeasonAlert>, ApiError> {
    let shows = get_watched_items(state.clone()).await?;
    let mut updates = Vec::new();
    for item in shows.iter().filter(|i| i.content_type == "tv") {
        if let Ok(details) = state
            .http
            .get::<TvDetailsFull>(&state, &format!("tv/{}", item.id), &[], false)
            .await
        {
            // Only aired seasons are alerts; legacy entries with no season tracking get a baseline.
            let aired: Vec<u32> = details
                .seasons
                .iter()
                .filter(|s| {
                    s.season_number > 0
                        && s.air_date.as_deref().is_some_and(|d| {
                            d <= chrono::Utc::now().format("%Y-%m-%d").to_string().as_str()
                        })
                })
                .map(|s| s.season_number)
                .collect();
            updates.push((item.id, details.number_of_seasons, aired));
        }
    }
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut items: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
    let mut alerts = Vec::new();
    for (id, total, aired) in updates {
        if let Some(item) = items
            .iter_mut()
            .find(|i| i.id == id && i.content_type == "tv")
        {
            let seen = item.watched_seasons.get_or_insert_with(|| aired.clone());
            let count = aired.iter().filter(|s| !seen.contains(s)).count() as u32;
            item.total_seasons_known = Some(total);
            item.has_new_seasons = Some(count > 0);
            if count > 0 {
                alerts.push(NewSeasonAlert {
                    id,
                    title: item.title.clone(),
                    new_season_count: count,
                    total_seasons: total,
                });
            }
        }
    }
    if !items.is_empty() {
        storage::write(&dir, "watched.json", &items)?;
    }
    Ok(alerts)
}
#[tauri::command]
fn disable_always_on_top(window: tauri::Window) -> Result<(), String> {
    window.set_always_on_top(false).map_err(|e| e.to_string())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(std::sync::Arc::new(AppState {
            api_key: Mutex::new(None),
            data_lock: Mutex::new(()),
            http: TmdbClient::new(),
            mcp: Mutex::new(ServerControl::default()),
        }))
        .invoke_handler(tauri::generate_handler![
            save_api_key,
            load_api_key,
            get_movie_genres,
            get_tv_genres,
            get_languages,
            get_watch_providers,
            search_movies,
            search_tv_shows,
            discovery::discover_titles_page,
            get_movie_details,
            get_tv_details,
            get_trailers,
            clear_api_cache,
            offline::offline_status,
            offline::set_offline_limit,
            offline::clear_offline_cache,
            offline::cache_image,
            offline::prepare_offline_library,
            add_watched_item,
            remove_watched_item,
            get_watched_items,
            is_watched,
            add_white_list_item,
            remove_white_list_item,
            get_white_list_items,
            is_white_listed,
            get_tv_season_details,
            update_watched_seasons,
            check_all_new_seasons,
            disable_always_on_top,
            get_ai_workspace,
            save_research,
            review_proposal,
            review_suggestion,
            dismiss_suggestion,
            start_mcp,
            stop_mcp,
            mcp_status,
            backup_library
        ])
        .run(tauri::generate_context!())
        .expect("error while running MoviNight");
}
