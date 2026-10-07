use crate::*;
use serde_json::{json, Value};
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AiWorkspace {
    #[serde(default)]
    pub research: Research,
    #[serde(default)]
    pub proposals: Vec<Proposal>,
    #[serde(default)]
    pub suggestions: Vec<Suggestion>,
}
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Research {
    pub id: String,
    pub text: String,
    pub instructions: String,
    pub updated_at: String,
    #[serde(default)]
    pub agent_note: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Proposal {
    pub proposal_id: String,
    pub research_id: String,
    pub requested_title: String,
    pub reason: String,
    pub item: WhiteListItem,
    pub created_at: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Suggestion {
    pub suggestion_id: String,
    pub source: String,
    pub reason: String,
    pub item: Value,
    pub created_at: String,
    #[serde(default = "pending_status")]
    pub status: String,
    #[serde(default)]
    pub recommended_seasons: Vec<u32>,
}
fn pending_status() -> String {
    "pending".into()
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn workspace(state: &AppState) -> Result<AiWorkspace, ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    Ok(storage::read(&get_config_dir()?, "ai_workspace.json")?.unwrap_or_default())
}
#[tauri::command]
pub fn get_ai_workspace(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<AiWorkspace, ApiError> {
    workspace(&state)
}
#[tauri::command]
pub fn save_research(
    text: String,
    instructions: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Research, ApiError> {
    if text.len() > 200_000 || instructions.len() > 10_000 {
        return Err(ApiError::Invalid(
            "Research supports up to 200 KB of text and 10 KB of instructions".into(),
        ));
    }
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut ws: AiWorkspace = storage::read(&dir, "ai_workspace.json")?.unwrap_or_default();
    // Keep the batch ID for unchanged text, so saving instructions does not orphan proposals.
    let id = if ws.research.text == text && !ws.research.id.is_empty() {
        ws.research.id.clone()
    } else {
        uuid::Uuid::new_v4().to_string()
    };
    ws.research = Research {
        id,
        text,
        instructions,
        updated_at: now(),
        agent_note: String::new(),
    };
    storage::write(&dir, "ai_workspace.json", &ws)?;
    Ok(ws.research)
}
pub async fn stage(
    state: &AppState,
    id: u32,
    kind: String,
    requested_title: String,
    reason: String,
    research_id: String,
) -> Result<Value, ApiError> {
    validate_type(&kind)?;
    if requested_title.trim().is_empty()
        || reason.trim().is_empty()
        || requested_title.len() > 500
        || reason.len() > 4000
    {
        return Err(ApiError::Invalid(
            "Provide the original title and a short matching explanation".into(),
        ));
    }
    let details: Value = state
        .http
        .get(state, &format!("{kind}/{id}"), &[], false)
        .await?;
    let title = details[if kind == "movie" { "title" } else { "name" }]
        .as_str()
        .ok_or_else(|| ApiError::Invalid("TMDB title unavailable".into()))?
        .to_owned();
    let item = WhiteListItem {
        extra: Default::default(),
        id,
        title,
        overview: details["overview"].as_str().unwrap_or("").into(),
        poster_path: details["poster_path"].as_str().map(str::to_owned),
        release_date: details[if kind == "movie" {
            "release_date"
        } else {
            "first_air_date"
        }]
        .as_str()
        .unwrap_or("")
        .into(),
        vote_average: details["vote_average"].as_f64().unwrap_or(0.0),
        content_type: kind,
        white_list_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
    };
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut ws: AiWorkspace = storage::read(&dir, "ai_workspace.json")?.unwrap_or_default();
    if ws.research.id != research_id || research_id.is_empty() {
        return Err(ApiError::Invalid(
            "Research batch changed. Read get_research again before proposing matches.".into(),
        ));
    }
    if let Some(p) = ws.proposals.iter().find(|p| {
        p.item.id == id && p.item.content_type == item.content_type && p.research_id == research_id
    }) {
        return Ok(json!({"status":"already_pending","proposal_id":p.proposal_id}));
    }
    if ws.proposals.len() >= 1000 {
        return Err(ApiError::Invalid(
            "Review existing proposals before adding more (limit 1000)".into(),
        ));
    }
    let proposal = Proposal {
        proposal_id: uuid::Uuid::new_v4().to_string(),
        research_id,
        requested_title,
        reason,
        item,
        created_at: now(),
    };
    let output = json!({"status":"pending_user_approval","proposal":proposal});
    ws.proposals.push(proposal);
    storage::write(&dir, "ai_workspace.json", &ws)?;
    Ok(output)
}
fn approve_item(
    list: &mut Vec<WhiteListItem>,
    watched: &[WatchedItem],
    item: &WhiteListItem,
) -> &'static str {
    if watched
        .iter()
        .any(|w| w.id == item.id && w.content_type == item.content_type)
    {
        list.retain(|i| !(i.id == item.id && i.content_type == item.content_type));
        return "watched";
    }
    if !list
        .iter()
        .any(|i| i.id == item.id && i.content_type == item.content_type)
    {
        let mut item = item.clone();
        item.white_list_date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        list.push(item);
    }
    "waitlist"
}
#[cfg(test)]
fn approve_into(list: &mut Vec<WhiteListItem>, proposal: &Proposal) {
    approve_item(list, &[], &proposal.item);
}
#[tauri::command]
pub fn review_proposal(
    proposal_id: String,
    approve: bool,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Value, ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut ws: AiWorkspace = storage::read(&dir, "ai_workspace.json")?.unwrap_or_default();
    let proposal = ws
        .proposals
        .iter()
        .find(|p| p.proposal_id == proposal_id)
        .ok_or_else(|| ApiError::Invalid("Proposal was already reviewed".into()))?
        .clone();
    let mut destination = "rejected";
    if approve {
        let mut list: Vec<WhiteListItem> =
            storage::read(&dir, "white_list.json")?.unwrap_or_default();
        let watched: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
        let before = list.len();
        destination = approve_item(&mut list, &watched, &proposal.item);
        // Library first. If queue persistence fails, approving again is idempotent.
        if destination == "waitlist" || before != list.len() {
            storage::write(&dir, "white_list.json", &list)?;
        }
        if !ws
            .suggestions
            .iter()
            .any(|s| s.suggestion_id == proposal.proposal_id)
        {
            ws.suggestions.push(Suggestion {
                suggestion_id: proposal.proposal_id.clone(),
                source: "research".into(),
                reason: proposal.reason.clone(),
                item: serde_json::to_value(&proposal.item)?,
                created_at: now(),
                status: "approved".into(),
                recommended_seasons: vec![],
            });
        }
    }
    ws.proposals.retain(|p| p.proposal_id != proposal_id);
    storage::write(&dir, "ai_workspace.json", &ws)?;
    Ok(json!({"status":if approve { "approved" } else { "rejected" },"destination":destination}))
}
pub fn suggest(
    state: &AppState,
    id: u32,
    kind: String,
    source: String,
    reason: String,
) -> Result<Value, ApiError> {
    validate_type(&kind)?;
    if reason.trim().is_empty() || reason.len() > 4000 {
        return Err(ApiError::Invalid(
            "Explain why this matches the user's preferences".into(),
        ));
    }
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let item = match source.as_str() {
        "watched" => {
            let items: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
            items
                .into_iter()
                .find(|i| i.id == id && i.content_type == kind)
                .map(|i| serde_json::to_value(i).unwrap())
        }
        "waitlist" => {
            let items: Vec<WhiteListItem> =
                storage::read(&dir, "white_list.json")?.unwrap_or_default();
            items
                .into_iter()
                .find(|i| i.id == id && i.content_type == kind)
                .map(|i| serde_json::to_value(i).unwrap())
        }
        _ => {
            return Err(ApiError::Invalid(
                "Source must be watched or waitlist".into(),
            ))
        }
    }
    .ok_or_else(|| ApiError::Invalid("This title is no longer in that library list".into()))?;
    let mut ws: AiWorkspace = storage::read(&dir, "ai_workspace.json")?.unwrap_or_default();
    if let Some(existing) = ws
        .suggestions
        .iter_mut()
        .find(|s| s.item["id"] == id && s.item["content_type"] == kind && s.status == "pending")
    {
        existing.reason = reason;
        existing.source = source;
        existing.item = item;
        existing.recommended_seasons.clear();
        let output = serde_json::to_value(&*existing)?;
        storage::write(&dir, "ai_workspace.json", &ws)?;
        return Ok(output);
    }
    if ws.suggestions.len() >= 1000 {
        return Err(ApiError::Invalid(
            "Dismiss older suggestions before publishing more (limit 1000)".into(),
        ));
    }
    let suggestion = Suggestion {
        suggestion_id: uuid::Uuid::new_v4().to_string(),
        source,
        reason,
        item,
        created_at: now(),
        status: pending_status(),
        recommended_seasons: vec![],
    };
    let output = serde_json::to_value(&suggestion)?;
    ws.suggestions.push(suggestion);
    storage::write(&dir, "ai_workspace.json", &ws)?;
    Ok(output)
}
pub async fn suggest_discovery(
    state: &AppState,
    id: u32,
    kind: String,
    reason: String,
    mut recommended_seasons: Vec<u32>,
) -> Result<Value, ApiError> {
    validate_type(&kind)?;
    if id == 0 || reason.trim().is_empty() || reason.len() > 4000 {
        return Err(ApiError::Invalid(
            "Provide a verified title ID and a useful reason (up to 4000 characters)".into(),
        ));
    }
    let details: Value = state
        .http
        .get(state, &format!("{kind}/{id}"), &[], false)
        .await?;
    if details["id"] != id {
        return Err(ApiError::Invalid(
            "TMDB did not confirm this title ID".into(),
        ));
    }
    let title = details[if kind == "movie" { "title" } else { "name" }]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ApiError::Invalid("TMDB title unavailable".into()))?;
    let item = WhiteListItem {
        id,
        title: title.into(),
        overview: details["overview"].as_str().unwrap_or("").into(),
        poster_path: details["poster_path"].as_str().map(str::to_owned),
        release_date: details[if kind == "movie" {
            "release_date"
        } else {
            "first_air_date"
        }]
        .as_str()
        .unwrap_or("")
        .into(),
        vote_average: details["vote_average"].as_f64().unwrap_or(0.0),
        content_type: kind.clone(),
        white_list_date: String::new(),
        extra: Default::default(),
    };
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let watched: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
    let history = watched
        .iter()
        .find(|w| w.id == id && w.content_type == kind);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let available = details["seasons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let number = s["season_number"].as_u64()? as u32;
            let date = s["air_date"].as_str()?;
            (number > 0
                && !date.is_empty()
                && date <= today.as_str()
                && history.is_some_and(|w| {
                    !w.watched_seasons
                        .as_ref()
                        .is_some_and(|seasons| seasons.contains(&number))
                }))
            .then_some(number)
        })
        .collect::<Vec<_>>();
    if recommended_seasons.len() > 100 || recommended_seasons.iter().any(|s| !available.contains(s))
    {
        return Err(ApiError::Invalid(
            "Season suggestions must be aired, unwatched seasons of a TV show in Watched".into(),
        ));
    }
    // Only automatically flag newly added seasons beyond a recorded known-season count.
    if recommended_seasons.is_empty() && kind == "tv" {
        if let Some(known) = history.and_then(|w| w.total_seasons_known) {
            recommended_seasons = available.into_iter().filter(|s| *s > known).collect();
        }
    }
    recommended_seasons.sort_unstable();
    recommended_seasons.dedup();
    let mut ws: AiWorkspace = storage::read(&dir, "ai_workspace.json")?.unwrap_or_default();
    if let Some(existing) = ws
        .suggestions
        .iter_mut()
        .find(|s| s.item["id"] == id && s.item["content_type"] == kind && s.status == "pending")
    {
        existing.reason = reason;
        existing.source = "discovery".into();
        existing.item = serde_json::to_value(&item)?;
        existing.recommended_seasons = recommended_seasons;
        let output = serde_json::to_value(&*existing)?;
        storage::write(&dir, "ai_workspace.json", &ws)?;
        return Ok(output);
    }
    if ws.suggestions.len() >= 1000 {
        return Err(ApiError::Invalid(
            "Dismiss older suggestions before publishing more (limit 1000)".into(),
        ));
    }
    let suggestion = Suggestion {
        suggestion_id: uuid::Uuid::new_v4().to_string(),
        source: "discovery".into(),
        reason,
        item: serde_json::to_value(item)?,
        created_at: now(),
        status: pending_status(),
        recommended_seasons,
    };
    let output = serde_json::to_value(&suggestion)?;
    ws.suggestions.push(suggestion);
    storage::write(&dir, "ai_workspace.json", &ws)?;
    Ok(output)
}

#[tauri::command]
pub fn review_suggestion(
    suggestion_id: String,
    approve: bool,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Value, ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    review_suggestion_at(&get_config_dir()?, &suggestion_id, approve)
}
fn review_suggestion_at(
    dir: &std::path::Path,
    suggestion_id: &str,
    approve: bool,
) -> Result<Value, ApiError> {
    let mut ws: AiWorkspace = storage::read(dir, "ai_workspace.json")?.unwrap_or_default();
    let suggestion = ws
        .suggestions
        .iter_mut()
        .find(|s| s.suggestion_id == suggestion_id)
        .ok_or_else(|| ApiError::Invalid("Suggestion was already removed".into()))?;
    if suggestion.status == "approved" {
        return Ok(json!({"status":"already_approved"}));
    }
    let mut destination = "rejected";
    if approve {
        let watched: Vec<WatchedItem> = storage::read(dir, "watched.json")?.unwrap_or_default();
        let mut list: Vec<WhiteListItem> =
            storage::read(dir, "white_list.json")?.unwrap_or_default();
        let mut value = suggestion.item.clone();
        value["white_list_date"] = json!("");
        let item: WhiteListItem = serde_json::from_value(value)?;
        let before = list.len();
        destination = approve_item(&mut list, &watched, &item);
        if destination == "waitlist" || before != list.len() {
            storage::write(dir, "white_list.json", &list)?;
        }
        // Keep an approved record without changing watched dates or tracked seasons.
        suggestion.status = "approved".into();
    } else {
        ws.suggestions.retain(|s| s.suggestion_id != suggestion_id);
    }
    storage::write(dir, "ai_workspace.json", &ws)?;
    Ok(json!({"status":if approve {"approved"} else {"rejected"},"destination":destination}))
}
#[tauri::command]
pub fn dismiss_suggestion(
    suggestion_id: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut ws: AiWorkspace = storage::read(&dir, "ai_workspace.json")?.unwrap_or_default();
    ws.suggestions.retain(|s| s.suggestion_id != suggestion_id);
    storage::write(&dir, "ai_workspace.json", &ws)
}
pub fn note(state: &AppState, research_id: String, note: String) -> Result<(), ApiError> {
    if note.len() > 10_000 {
        return Err(ApiError::Invalid("Progress note is too long".into()));
    }
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut ws: AiWorkspace = storage::read(&dir, "ai_workspace.json")?.unwrap_or_default();
    if ws.research.id != research_id {
        return Err(ApiError::Invalid("Research batch changed".into()));
    }
    ws.research.agent_note = note;
    storage::write(&dir, "ai_workspace.json", &ws)
}
#[tauri::command]
pub fn backup_library(state: State<'_, std::sync::Arc<AppState>>) -> Result<String, ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    storage::backup(&get_config_dir()?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suggestions_recheck_watched_status_and_preserve_seasons_and_approved_history() {
        let dir = tempfile::tempdir().unwrap();
        let item = json!({"id":42,"title":"Series","overview":"Plot","poster_path":null,"release_date":"2000-01-01","vote_average":8,"content_type":"tv","white_list_date":"2020-01-02"});
        let mut history = item.clone();
        history.as_object_mut().unwrap().remove("white_list_date");
        history["watched_date"] = json!("2020-02-03");
        history["watched_seasons"] = json!([1, 2]);
        history["total_seasons_known"] = json!(3);
        history["personal_note"] = json!("Keep this");
        let suggestion = Suggestion {
            suggestion_id: "s".into(),
            source: "discovery".into(),
            reason: "Catch up on season 3".into(),
            item: item.clone(),
            created_at: now(),
            status: pending_status(),
            recommended_seasons: vec![3],
        };
        storage::write(
            dir.path(),
            "ai_workspace.json",
            &AiWorkspace {
                suggestions: vec![suggestion],
                ..Default::default()
            },
        )
        .unwrap();
        // The title became watched after the recommendation was published. An old duplicate may also exist in Waitlist.
        storage::write(dir.path(), "watched.json", &json!([history])).unwrap();
        let original = std::fs::read(dir.path().join("watched.json")).unwrap();
        storage::write(dir.path(), "white_list.json", &json!([item])).unwrap();
        let result = review_suggestion_at(dir.path(), "s", true).unwrap();
        assert_eq!(result["destination"], "watched");
        let list: Vec<WhiteListItem> = storage::read(dir.path(), "white_list.json")
            .unwrap()
            .unwrap();
        assert!(list.is_empty());
        assert_eq!(
            std::fs::read(dir.path().join("watched.json")).unwrap(),
            original
        );
        let ws: AiWorkspace = storage::read(dir.path(), "ai_workspace.json")
            .unwrap()
            .unwrap();
        assert_eq!(ws.suggestions[0].status, "approved");
        assert_eq!(ws.suggestions[0].recommended_seasons, vec![3]);
        assert_eq!(
            review_suggestion_at(dir.path(), "s", true).unwrap()["status"],
            "already_approved"
        );
        assert_eq!(
            std::fs::read(dir.path().join("watched.json")).unwrap(),
            original
        );
    }
    #[test]
    fn new_title_approval_is_idempotent_and_rejection_does_not_touch_library() {
        let dir = tempfile::tempdir().unwrap();
        let item = json!({"id":42,"title":"Movie","overview":"Plot","poster_path":null,"release_date":"2000-01-01","vote_average":8,"content_type":"movie","white_list_date":""});
        let suggestion = Suggestion {
            suggestion_id: "s".into(),
            source: "discovery".into(),
            reason: "Good fit".into(),
            item: item.clone(),
            created_at: now(),
            status: pending_status(),
            recommended_seasons: vec![],
        };
        storage::write(
            dir.path(),
            "ai_workspace.json",
            &AiWorkspace {
                suggestions: vec![suggestion.clone()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            review_suggestion_at(dir.path(), "s", true).unwrap()["destination"],
            "waitlist"
        );
        review_suggestion_at(dir.path(), "s", true).unwrap();
        let list: Vec<WhiteListItem> = storage::read(dir.path(), "white_list.json")
            .unwrap()
            .unwrap();
        assert_eq!(list.len(), 1);
        let before = std::fs::read(dir.path().join("white_list.json")).unwrap();
        storage::write(
            dir.path(),
            "ai_workspace.json",
            &AiWorkspace {
                suggestions: vec![suggestion],
                ..Default::default()
            },
        )
        .unwrap();
        review_suggestion_at(dir.path(), "s", false).unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("white_list.json")).unwrap(),
            before
        );
        let ws: AiWorkspace = storage::read(dir.path(), "ai_workspace.json")
            .unwrap()
            .unwrap();
        assert!(ws.suggestions.is_empty());
    }
    #[test]
    fn approval_is_idempotent_and_keeps_existing_dates() {
        let item = WhiteListItem {
            extra: Default::default(),
            id: 42,
            title: "Matched".into(),
            overview: "".into(),
            poster_path: None,
            release_date: "2000".into(),
            vote_average: 8.0,
            content_type: "movie".into(),
            white_list_date: "2020-01-02".into(),
        };
        let p = Proposal {
            proposal_id: "p".into(),
            research_id: "r".into(),
            requested_title: "Original".into(),
            reason: "Exact match".into(),
            item: item.clone(),
            created_at: "".into(),
        };
        let mut list = vec![item];
        approve_into(&mut list, &p);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].white_list_date, "2020-01-02");
        let mut empty = vec![];
        assert!(empty.is_empty());
        approve_into(&mut empty, &p);
        approve_into(&mut empty, &p);
        assert_eq!(empty.len(), 1);
    }
}
