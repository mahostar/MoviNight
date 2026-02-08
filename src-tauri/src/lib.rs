// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::State;

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
    release_date: String, // For movies it's release_date, for TV shows it's first_air_date
    vote_average: f64,
    content_type: String, // "movie" or "tv"
    watched_date: String, // ISO date when marked as watched
}

// White list item structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct WhiteListItem {
    id: u32,
    title: String,
    overview: String,
    poster_path: Option<String>,
    release_date: String, // For movies it's release_date, for TV shows it's first_air_date
    vote_average: f64,
    content_type: String, // "movie" or "tv"
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
    first_air_date: String,
    vote_average: f64,
    vote_count: u32,
    genres: Vec<Genre>,
    number_of_seasons: u32,
    number_of_episodes: u32,
    tagline: Option<String>,
}

// Application state for storing API key and watched items
struct AppState {
    api_key: std::sync::Mutex<Option<String>>,
    watched_items: std::sync::Mutex<Vec<WatchedItem>>,
    white_list_items: std::sync::Mutex<Vec<WhiteListItem>>,
}

// Error handling as recommended in the documentation
#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("No API key found. Please set your TMDB API key in settings.")]
    NoApiKey,
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Failed to parse response: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("File system error: {0}")]
    FileError(#[from] std::io::Error),
}

impl serde::Serialize for ApiError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

fn get_config_dir() -> Result<PathBuf, ApiError> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Config directory not found"))?
        .join("movinight");
    
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }
    
    Ok(config_dir)
}

// Commands following Tauri v2 documentation patterns

#[tauri::command]
async fn save_api_key(api_key: String, state: State<'_, AppState>) -> Result<(), ApiError> {
    // Save to state
    let mut stored_key = state.api_key.lock().unwrap();
    *stored_key = Some(api_key.clone());
    
    // Save to file for persistence
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join("config.json");
    
    let config = serde_json::json!({
        "api_key": api_key
    });
    
    fs::write(config_file, serde_json::to_string_pretty(&config)?)?;
    
    Ok(())
}

#[tauri::command]
async fn load_api_key(state: State<'_, AppState>) -> Result<Option<String>, ApiError> {
    // First check in-memory state
    {
        let stored_key = state.api_key.lock().unwrap();
        if stored_key.is_some() {
            return Ok(stored_key.clone());
        }
    }
    
    // Load from file
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join("config.json");
    
    if config_file.exists() {
        let content = fs::read_to_string(config_file)?;
        let config: serde_json::Value = serde_json::from_str(&content)?;
        
        if let Some(api_key) = config.get("api_key").and_then(|k| k.as_str()) {
            let api_key = api_key.to_string();
            
            // Update in-memory state
            let mut stored_key = state.api_key.lock().unwrap();
            *stored_key = Some(api_key.clone());
            
            return Ok(Some(api_key));
        }
    }
    
    Ok(None)
}

// Watched items management commands
#[tauri::command]
async fn add_watched_item(
    id: u32,
    title: String,
    overview: String,
    poster_path: Option<String>,
    release_date: String,
    vote_average: f64,
    content_type: String,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    let watched_item = WatchedItem {
        id,
        title,
        overview,
        poster_path,
        release_date,
        vote_average,
        content_type: content_type.clone(),
        watched_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
    };

    // Add to in-memory state
    {
        let mut watched_items = state.watched_items.lock().unwrap();
        // Remove if already exists (to avoid duplicates)
        watched_items.retain(|item| !(item.id == id && item.content_type == content_type));
        watched_items.push(watched_item.clone());
    }

    // Save to file
    let config_dir = get_config_dir()?;
    let watched_file = config_dir.join("watched.json");
    
    let watched_items = state.watched_items.lock().unwrap();
    fs::write(watched_file, serde_json::to_string_pretty(&*watched_items)?)?;
    
    Ok(())
}

#[tauri::command]
async fn remove_watched_item(
    id: u32,
    content_type: String,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    // Remove from in-memory state
    {
        let mut watched_items = state.watched_items.lock().unwrap();
        watched_items.retain(|item| !(item.id == id && item.content_type == content_type));
    }

    // Save to file
    let config_dir = get_config_dir()?;
    let watched_file = config_dir.join("watched.json");
    
    let watched_items = state.watched_items.lock().unwrap();
    fs::write(watched_file, serde_json::to_string_pretty(&*watched_items)?)?;
    
    Ok(())
}

#[tauri::command]
async fn get_watched_items(state: State<'_, AppState>) -> Result<Vec<WatchedItem>, ApiError> {
    // First check in-memory state
    {
        let watched_items = state.watched_items.lock().unwrap();
        if !watched_items.is_empty() {
            return Ok(watched_items.clone());
        }
    }
    
    // Load from file
    let config_dir = get_config_dir()?;
    let watched_file = config_dir.join("watched.json");
    
    if watched_file.exists() {
        let content = fs::read_to_string(watched_file)?;
        let watched_items: Vec<WatchedItem> = serde_json::from_str(&content)?;
        
        // Update in-memory state
        {
            let mut stored_items = state.watched_items.lock().unwrap();
            *stored_items = watched_items.clone();
        }
        
        Ok(watched_items)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
async fn is_watched(
    id: u32,
    content_type: String,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    // First ensure we have loaded watched items
    let _ = get_watched_items(state.clone()).await?;

    let watched_items = state.watched_items.lock().unwrap();
    let is_watched = watched_items.iter().any(|item| item.id == id && item.content_type == content_type);

    Ok(is_watched)
}

// White list items management commands
#[tauri::command]
async fn add_white_list_item(
    id: u32,
    title: String,
    overview: String,
    poster_path: Option<String>,
    release_date: String,
    vote_average: f64,
    content_type: String,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    let white_list_item = WhiteListItem {
        id,
        title,
        overview,
        poster_path,
        release_date,
        vote_average,
        content_type: content_type.clone(),
        white_list_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
    };

    // Add to in-memory state
    {
        let mut white_list_items = state.white_list_items.lock().unwrap();
        // Remove if already exists (to avoid duplicates)
        white_list_items.retain(|item| !(item.id == id && item.content_type == content_type));
        white_list_items.push(white_list_item.clone());
    }

    // Save to file
    let config_dir = get_config_dir()?;
    let white_list_file = config_dir.join("white_list.json");
    
    let white_list_items = state.white_list_items.lock().unwrap();
    fs::write(white_list_file, serde_json::to_string_pretty(&*white_list_items)?)?;

    Ok(())
}

#[tauri::command]
async fn remove_white_list_item(
    id: u32,
    content_type: String,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    // Remove from in-memory state
    {
        let mut white_list_items = state.white_list_items.lock().unwrap();
        white_list_items.retain(|item| !(item.id == id && item.content_type == content_type));
    }

    // Save to file
    let config_dir = get_config_dir()?;
    let white_list_file = config_dir.join("white_list.json");
    
    let white_list_items = state.white_list_items.lock().unwrap();
    fs::write(white_list_file, serde_json::to_string_pretty(&*white_list_items)?)?;

    Ok(())
}

#[tauri::command]
async fn get_white_list_items(state: State<'_, AppState>) -> Result<Vec<WhiteListItem>, ApiError> {
    // First check in-memory state
    {
        let white_list_items = state.white_list_items.lock().unwrap();
        if !white_list_items.is_empty() {
            return Ok(white_list_items.clone());
        }
    }
    
    // Load from file
    let config_dir = get_config_dir()?;
    let white_list_file = config_dir.join("white_list.json");

    if white_list_file.exists() {
        let content = fs::read_to_string(white_list_file)?;
        let white_list_items: Vec<WhiteListItem> = serde_json::from_str(&content)?;

        // Update in-memory state
        {
            let mut stored_items = state.white_list_items.lock().unwrap();
            *stored_items = white_list_items.clone();
        }

        Ok(white_list_items)
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
async fn is_white_listed(
    id: u32,
    content_type: String,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    // First ensure we have loaded white list items
    let _ = get_white_list_items(state.clone()).await?;

    let white_list_items = state.white_list_items.lock().unwrap();
    let is_white_listed = white_list_items.iter().any(|item| item.id == id && item.content_type == content_type);

    Ok(is_white_listed)
}

#[tauri::command]
async fn get_movie_genres(state: State<'_, AppState>) -> Result<Vec<Genre>, ApiError> {
    let api_key = {
        let stored_key = state.api_key.lock().unwrap();
        stored_key.clone().ok_or(ApiError::NoApiKey)?
    };

    let url = format!(
        "https://api.themoviedb.org/3/genre/movie/list?api_key={}&language=en-US",
        api_key
    );

    let response = reqwest::get(&url).await?;
    let genre_response: GenreResponse = response.json().await?;

    Ok(genre_response.genres)
}

#[tauri::command]
async fn get_tv_genres(state: State<'_, AppState>) -> Result<Vec<Genre>, ApiError> {
    let api_key = {
        let stored_key = state.api_key.lock().unwrap();
        stored_key.clone().ok_or(ApiError::NoApiKey)?
    };
    
    let url = format!(
        "https://api.themoviedb.org/3/genre/tv/list?api_key={}&language=en-US",
        api_key
    );
    
    let response = reqwest::get(&url).await?;
    let genre_response: GenreResponse = response.json().await?;

    Ok(genre_response.genres)
}

#[tauri::command]
async fn get_languages(state: State<'_, AppState>) -> Result<Vec<Language>, ApiError> {
    let api_key = {
        let stored_key = state.api_key.lock().unwrap();
        stored_key.clone().ok_or(ApiError::NoApiKey)?
    };
    
    let url = format!(
        "https://api.themoviedb.org/3/configuration/languages?api_key={}",
        api_key
    );
    
    let response = reqwest::get(&url).await?;
    let languages: Vec<Language> = response.json().await?;
    
    Ok(languages)
}

#[tauri::command]
async fn get_watch_providers(content_type: String, state: State<'_, AppState>) -> Result<Vec<WatchProvider>, ApiError> {
    let api_key = {
        let stored_key = state.api_key.lock().unwrap();
        stored_key.clone().ok_or(ApiError::NoApiKey)?
    };

    let path = if content_type == "movie" { "movie" } else { "tv" };
    
    let url = format!(
        "https://api.themoviedb.org/3/watch/providers/{}?api_key={}&language=en-US&watch_region=US",
        path, api_key
    );
    
    let response = reqwest::get(&url).await?;
    let provider_response: WatchProviderResponse = response.json().await?;
    
    Ok(provider_response.results)
}

#[tauri::command]
async fn search_movies(
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
    state: State<'_, AppState>,
) -> Result<ApiResponse<Movie>, ApiError> {
    let api_key = {
        let stored_key = state.api_key.lock().unwrap();
        stored_key.clone().ok_or(ApiError::NoApiKey)?
    };
    
    let mut url = if query.is_empty() {
        // Discover movies
            format!(
            "https://api.themoviedb.org/3/discover/movie?api_key={}&language=en-US&page={}&sort_by={}",
            api_key, page, sort_by
            )
        } else {
        // Search movies
            format!(
            "https://api.themoviedb.org/3/search/movie?api_key={}&language=en-US&query={}&page={}",
            api_key, urlencoding::encode(&query), page
        )
    };
    
    // year_from is the START year (left side), year_to is the END year (right side)
    // For TMDB API: gte = greater than or equal (start date), lte = less than or equal (end date)
    if let Some(year) = year_from {
        url.push_str(&format!("&primary_release_date.gte={}-01-01", year));
    }
    if let Some(year) = year_to {
        url.push_str(&format!("&primary_release_date.lte={}-12-31", year));
    }
    
    if !genre_ids.is_empty() {
        let genres = genre_ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        url.push_str(&format!("&with_genres={}", genres));
    }
    
    // Exclude animation genre (ID 16) if requested
    if exclude_animation {
        url.push_str("&without_genres=16");
    }

    if !with_watch_providers.is_empty() {
        let providers = with_watch_providers.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join("|"); // OR logic
        url.push_str(&format!("&with_watch_providers={}&watch_region=US", providers));
    }

    if let Some(lang) = with_original_language {
        if !lang.is_empty() {
            url.push_str(&format!("&with_original_language={}", lang));
        }
    }

    // Add minimum rating filter (vote_average.gte)
    if let Some(rating) = min_rating {
        if rating > 0.0 {
            url.push_str(&format!("&vote_average.gte={}", rating));
        }
    }
    
    let response = reqwest::get(&url).await?;
    let api_response: ApiResponse<Movie> = response.json().await?;
    
    Ok(api_response)
}

#[tauri::command]
async fn search_tv_shows(
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
    state: State<'_, AppState>,
) -> Result<ApiResponse<TvShow>, ApiError> {
    let api_key = {
        let stored_key = state.api_key.lock().unwrap();
        stored_key.clone().ok_or(ApiError::NoApiKey)?
    };
    
    let mut url = if query.is_empty() {
        // Discover TV shows
        format!(
            "https://api.themoviedb.org/3/discover/tv?api_key={}&language=en-US&page={}&sort_by={}",
            api_key, page, sort_by
        )
    } else {
        // Search TV shows
        format!(
            "https://api.themoviedb.org/3/search/tv?api_key={}&language=en-US&query={}&page={}",
            api_key, urlencoding::encode(&query), page
        )
    };

    // year_from is the START year (left side), year_to is the END year (right side)
    // For TMDB API: gte = greater than or equal (start date), lte = less than or equal (end date)
    if let Some(year) = year_from {
        url.push_str(&format!("&air_date.gte={}-01-01", year));
    }
    if let Some(year) = year_to {
        url.push_str(&format!("&air_date.lte={}-12-31", year));
    }
    
    if !genre_ids.is_empty() {
        let genres = genre_ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        url.push_str(&format!("&with_genres={}", genres));
    }

    // Exclude animation genre (ID 16) for TV shows if requested
    if exclude_animation {
        url.push_str("&without_genres=16");
    }

    if !with_watch_providers.is_empty() {
        let providers = with_watch_providers.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join("|"); // OR logic
        url.push_str(&format!("&with_watch_providers={}&watch_region=US", providers));
    }

    if let Some(lang) = with_original_language {
        if !lang.is_empty() {
            url.push_str(&format!("&with_original_language={}", lang));
        }
    }

    // Add minimum rating filter (vote_average.gte)
    if let Some(rating) = min_rating {
        if rating > 0.0 {
            url.push_str(&format!("&vote_average.gte={}", rating));
        }
    }
    
    let response = reqwest::get(&url).await?;
    let api_response: ApiResponse<TvShow> = response.json().await?;

    Ok(api_response)
}

// Get movie details with trailers
#[tauri::command]
async fn get_movie_details(id: u32, state: State<'_, AppState>) -> Result<MovieDetails, ApiError> {
    let api_key = {
        let stored_key = state.api_key.lock().unwrap();
        stored_key.clone().ok_or(ApiError::NoApiKey)?
    };

    let client = reqwest::Client::new();
    let url = format!(
        "https://api.themoviedb.org/3/movie/{}?api_key={}&language=en-US",
        id, api_key
    );

    let response = client.get(&url).send().await?;
    let movie_details: MovieDetails = response.json().await?;

    Ok(movie_details)
}

// Get TV show details with trailers
#[tauri::command]
async fn get_tv_details(id: u32, state: State<'_, AppState>) -> Result<TvDetails, ApiError> {
    let api_key = {
        let stored_key = state.api_key.lock().unwrap();
        stored_key.clone().ok_or(ApiError::NoApiKey)?
    };

    let client = reqwest::Client::new();
    let url = format!(
        "https://api.themoviedb.org/3/tv/{}?api_key={}&language=en-US",
        id, api_key
    );

    let response = client.get(&url).send().await?;
    let tv_details: TvDetails = response.json().await?;

    Ok(tv_details)
}

// Get trailers for a movie or TV show
#[tauri::command]
async fn get_trailers(id: u32, content_type: String, state: State<'_, AppState>) -> Result<Vec<Trailer>, ApiError> {
    let api_key = {
        let stored_key = state.api_key.lock().unwrap();
        stored_key.clone().ok_or(ApiError::NoApiKey)?
    };

    let client = reqwest::Client::new();
    let endpoint = if content_type == "movie" { "movie" } else { "tv" };
    let url = format!(
        "https://api.themoviedb.org/3/{}/{}/videos?api_key={}&language=en-US",
        endpoint, id, api_key
    );

    let response = client.get(&url).send().await?;
    let trailer_response: TrailerResponse = response.json().await?;

    // Filter for YouTube trailers only and prioritize official trailers
    let mut trailers: Vec<Trailer> = trailer_response.results
        .into_iter()
        .filter(|t| t.site == "YouTube" && (t.trailer_type == "Trailer" || t.trailer_type == "Teaser"))
        .collect();

    // Sort by official status (official first) and then by published date (newest first)
    trailers.sort_by(|a, b| {
        match b.official.cmp(&a.official) {
            std::cmp::Ordering::Equal => b.published_at.cmp(&a.published_at),
            other => other,
        }
    });

    Ok(trailers)
}

/// Disables the always on top flag for the main window.
#[tauri::command]
async fn disable_always_on_top(window: tauri::Window) -> Result<(), String> {
  window.set_always_on_top(false).map_err(|e| e.to_string())?;
  Ok(())
}

// Simple greet command for testing
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to MoviNight! 🎬", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            api_key: std::sync::Mutex::new(None),
            watched_items: std::sync::Mutex::new(Vec::new()),
            white_list_items: std::sync::Mutex::new(Vec::new()),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            save_api_key,
            load_api_key,
            get_movie_genres,
            get_tv_genres,
            get_languages,
            get_watch_providers,
            search_movies,
            search_tv_shows,
            add_watched_item,
            remove_watched_item,
            get_watched_items,
            is_watched,
            add_white_list_item,
            remove_white_list_item,
            get_white_list_items,
            is_white_listed,
            get_movie_details,
            get_tv_details,
            get_trailers,
            disable_always_on_top
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}