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
    let list: Vec<WhiteListItem> = storage::read(&dir, "white_list.json")?.unwrap_or_default();
    if list
        .iter()
        .any(|i| i.id == id && i.content_type == item.content_type)
    {
        return Ok(json!({"status":"already_in_waitlist","id":id}));
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
fn approve_into(list: &mut Vec<WhiteListItem>, proposal: &Proposal) {
    if !list
        .iter()
        .any(|i| i.id == proposal.item.id && i.content_type == proposal.item.content_type)
    {
        let mut item = proposal.item.clone();
        item.white_list_date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        list.push(item);
    }
}
#[tauri::command]
pub fn review_proposal(
    proposal_id: String,
    approve: bool,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut ws: AiWorkspace = storage::read(&dir, "ai_workspace.json")?.unwrap_or_default();
    let proposal = ws
        .proposals
        .iter()
        .find(|p| p.proposal_id == proposal_id)
        .ok_or_else(|| ApiError::Invalid("Proposal was already reviewed".into()))?
        .clone();
    if approve {
        let mut list: Vec<WhiteListItem> =
            storage::read(&dir, "white_list.json")?.unwrap_or_default();
        approve_into(&mut list, &proposal);
        // Library first. If queue persistence fails, approving again is idempotent.
        storage::write(&dir, "white_list.json", &list)?;
    }
    ws.proposals.retain(|p| p.proposal_id != proposal_id);
    storage::write(&dir, "ai_workspace.json", &ws)
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
    ws.suggestions
        .retain(|s| !(s.item["id"] == id && s.item["content_type"] == kind && s.source == source));
    if ws.suggestions.len() >= 100 {
        ws.suggestions.remove(0);
    }
    let suggestion = Suggestion {
        suggestion_id: uuid::Uuid::new_v4().to_string(),
        source,
        reason,
        item,
        created_at: now(),
    };
    let output = serde_json::to_value(&suggestion)?;
    ws.suggestions.push(suggestion);
    storage::write(&dir, "ai_workspace.json", &ws)?;
    Ok(output)
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
