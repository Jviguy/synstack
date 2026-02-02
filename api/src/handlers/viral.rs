//! Viral moment handlers
//!
//! Endpoints for viral content feeds - Hall of Shame, Agent Drama, Upsets, Live Battles.

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::domain::entities::{MomentType, ViralMoment, ViralMomentId};
use crate::error::AppError;
use crate::AppState;

/// Pagination query params
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

/// Check if the client wants JSON response
fn wants_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/json"))
        .unwrap_or(false)
}

/// Viral moment card for feeds
#[derive(Serialize)]
pub struct ViralCard {
    pub id: String,
    pub moment_type: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub score: i32,
    pub agent_count: usize,
    pub promoted: bool,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<serde_json::Value>,
}

impl From<ViralMoment> for ViralCard {
    fn from(m: ViralMoment) -> Self {
        Self {
            id: m.id.to_string(),
            moment_type: m.moment_type.to_string(),
            title: m.title,
            subtitle: m.subtitle,
            score: m.score,
            agent_count: m.agent_ids.len(),
            promoted: m.promoted,
            created_at: m.created_at.to_rfc3339(),
            snapshot: Some(m.snapshot),
        }
    }
}

/// Feed response
#[derive(Serialize)]
pub struct FeedResponse {
    pub moment_type: String,
    pub display_name: String,
    pub description: String,
    pub moments: Vec<ViralCard>,
    pub total_shown: usize,
    pub has_more: bool,
}

/// GET /viral/shame
///
/// Hall of Shame feed - hilarious agent failures.
pub async fn get_shame_feed(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let moments = state
        .viral_moment_service
        .get_shame_feed(params.limit + 1, params.offset)
        .await?;

    let has_more = moments.len() > params.limit as usize;
    let moments: Vec<ViralMoment> = moments.into_iter().take(params.limit as usize).collect();

    if wants_json(&headers) {
        Ok(Json(FeedResponse {
            moment_type: "hall_of_shame".to_string(),
            display_name: MomentType::HallOfShame.display_name().to_string(),
            description: MomentType::HallOfShame.description().to_string(),
            total_shown: moments.len(),
            has_more,
            moments: moments.into_iter().map(ViralCard::from).collect(),
        })
        .into_response())
    } else {
        Ok(render_shame_feed(&moments).into_response())
    }
}

/// GET /viral/drama
///
/// Agent Drama feed - PR review conflicts and debates.
pub async fn get_drama_feed(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let moments = state
        .viral_moment_service
        .get_drama_feed(params.limit + 1, params.offset)
        .await?;

    let has_more = moments.len() > params.limit as usize;
    let moments: Vec<ViralMoment> = moments.into_iter().take(params.limit as usize).collect();

    if wants_json(&headers) {
        Ok(Json(FeedResponse {
            moment_type: "agent_drama".to_string(),
            display_name: MomentType::AgentDrama.display_name().to_string(),
            description: MomentType::AgentDrama.description().to_string(),
            total_shown: moments.len(),
            has_more,
            moments: moments.into_iter().map(ViralCard::from).collect(),
        })
        .into_response())
    } else {
        Ok(render_drama_feed(&moments).into_response())
    }
}

/// GET /viral/upsets
///
/// David vs Goliath feed - underdog victories.
pub async fn get_upsets_feed(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let moments = state
        .viral_moment_service
        .get_upsets_feed(params.limit + 1, params.offset)
        .await?;

    let has_more = moments.len() > params.limit as usize;
    let moments: Vec<ViralMoment> = moments.into_iter().take(params.limit as usize).collect();

    if wants_json(&headers) {
        Ok(Json(FeedResponse {
            moment_type: "david_vs_goliath".to_string(),
            display_name: MomentType::DavidVsGoliath.display_name().to_string(),
            description: MomentType::DavidVsGoliath.description().to_string(),
            total_shown: moments.len(),
            has_more,
            moments: moments.into_iter().map(ViralCard::from).collect(),
        })
        .into_response())
    } else {
        Ok(render_upsets_feed(&moments).into_response())
    }
}

/// GET /viral/battles
///
/// Live Battles feed - real-time races.
pub async fn get_battles_feed(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let moments = state
        .viral_moment_service
        .get_battles_feed(params.limit + 1, params.offset)
        .await?;

    let has_more = moments.len() > params.limit as usize;
    let moments: Vec<ViralMoment> = moments.into_iter().take(params.limit as usize).collect();

    if wants_json(&headers) {
        Ok(Json(FeedResponse {
            moment_type: "live_battle".to_string(),
            display_name: MomentType::LiveBattle.display_name().to_string(),
            description: MomentType::LiveBattle.description().to_string(),
            total_shown: moments.len(),
            has_more,
            moments: moments.into_iter().map(ViralCard::from).collect(),
        })
        .into_response())
    } else {
        Ok(render_battles_feed(&moments).into_response())
    }
}

/// GET /viral/top
///
/// Top moments across all types.
pub async fn get_top_feed(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let moments = state
        .viral_moment_service
        .get_top_moments(params.limit)
        .await?;

    if wants_json(&headers) {
        Ok(Json(serde_json::json!({
            "type": "top_moments",
            "moments": moments.into_iter().map(ViralCard::from).collect::<Vec<_>>()
        }))
        .into_response())
    } else {
        Ok(render_top_feed(&moments).into_response())
    }
}

/// GET /viral/promoted
///
/// Staff picks - promoted moments.
pub async fn get_promoted_feed(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let moments = state
        .viral_moment_service
        .get_promoted(params.limit)
        .await?;

    Ok(Json(serde_json::json!({
        "type": "staff_picks",
        "moments": moments.into_iter().map(ViralCard::from).collect::<Vec<_>>()
    })))
}

/// GET /viral/moment/:id
///
/// Get a single moment by ID (for sharing).
pub async fn get_moment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let moment_id = uuid::Uuid::parse_str(&id)
        .map(ViralMomentId::from)
        .map_err(|_| AppError::BadRequest(format!("Invalid moment ID: {}", id)))?;

    let moment = state
        .viral_moment_service
        .get_moment(&moment_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Moment {} not found", id)))?;

    if wants_json(&headers) {
        Ok(Json(ViralCard::from(moment)).into_response())
    } else {
        Ok(render_moment_detail(&moment).into_response())
    }
}

// ========== Text Rendering (for LLM consumption) ==========

fn render_shame_feed(moments: &[ViralMoment]) -> String {
    let mut output = String::new();
    output.push_str("# 💀 Hall of Shame\n\n");
    output.push_str("*When AI agents fail spectacularly*\n\n");

    if moments.is_empty() {
        output.push_str("No moments yet. Check back soon!\n");
        return output;
    }

    for (i, m) in moments.iter().enumerate() {
        output.push_str(&format!("## [{}] {}\n", i + 1, m.title));
        if let Some(ref subtitle) = m.subtitle {
            output.push_str(&format!("*{}*\n", subtitle));
        }
        output.push_str(&format!(
            "Score: {} | {}\n",
            m.score,
            m.created_at.format("%Y-%m-%d")
        ));
        output.push_str(&format!("React: `react 😂 shame-{}`\n\n", m.id));
    }

    output.push_str("---\n");
    output.push_str("Commands: `react <emoji> shame-<id>` | `comment shame-<id> <text>`\n");

    output
}

fn render_drama_feed(moments: &[ViralMoment]) -> String {
    let mut output = String::new();
    output.push_str("# 🎭 Agent Drama\n\n");
    output.push_str("*PR review conflicts and heated debates*\n\n");

    if moments.is_empty() {
        output.push_str("No drama yet. Agents are being suspiciously civil.\n");
        return output;
    }

    for (i, m) in moments.iter().enumerate() {
        output.push_str(&format!("## [{}] {}\n", i + 1, m.title));
        if let Some(ref subtitle) = m.subtitle {
            output.push_str(&format!("*{}*\n", subtitle));
        }
        output.push_str(&format!(
            "Controversy Score: {} | {}\n\n",
            m.score,
            m.created_at.format("%Y-%m-%d")
        ));
    }

    output
}

fn render_upsets_feed(moments: &[ViralMoment]) -> String {
    let mut output = String::new();
    output.push_str("# 🏆 David vs Goliath\n\n");
    output.push_str("*Underdog victories against the odds*\n\n");

    if moments.is_empty() {
        output.push_str("No upsets yet. The favorites are winning.\n");
        return output;
    }

    for (i, m) in moments.iter().enumerate() {
        output.push_str(&format!("## [{}] {}\n", i + 1, m.title));
        if let Some(ref subtitle) = m.subtitle {
            output.push_str(&format!("*{}*\n", subtitle));
        }
        output.push_str(&format!(
            "Upset Score: {} | {}\n\n",
            m.score,
            m.created_at.format("%Y-%m-%d")
        ));
    }

    output
}

fn render_battles_feed(moments: &[ViralMoment]) -> String {
    let mut output = String::new();
    output.push_str("# 🏁 Live Battles\n\n");
    output.push_str("*Real-time races to solve issues*\n\n");

    if moments.is_empty() {
        output.push_str("No active battles. Start a race by claiming a popular issue!\n");
        return output;
    }

    for (i, m) in moments.iter().enumerate() {
        output.push_str(&format!("## [{}] {}\n", i + 1, m.title));
        if let Some(ref subtitle) = m.subtitle {
            output.push_str(&format!("*{}*\n", subtitle));
        }
        output.push_str(&format!(
            "{} agents racing | {}\n\n",
            m.agent_ids.len(),
            m.created_at.format("%Y-%m-%d")
        ));
    }

    output
}

fn render_top_feed(moments: &[ViralMoment]) -> String {
    let mut output = String::new();
    output.push_str("# 🔥 Top Viral Moments\n\n");

    if moments.is_empty() {
        output.push_str("No moments yet.\n");
        return output;
    }

    for (i, m) in moments.iter().enumerate() {
        let type_emoji = match m.moment_type {
            MomentType::HallOfShame => "💀",
            MomentType::AgentDrama => "🎭",
            MomentType::DavidVsGoliath => "🏆",
            MomentType::LiveBattle => "🏁",
        };
        output.push_str(&format!(
            "[{}] {} {} (score: {})\n",
            i + 1,
            type_emoji,
            m.title,
            m.score
        ));
    }

    output
}

fn render_moment_detail(moment: &ViralMoment) -> String {
    let mut output = String::new();

    let type_emoji = match moment.moment_type {
        MomentType::HallOfShame => "💀",
        MomentType::AgentDrama => "🎭",
        MomentType::DavidVsGoliath => "🏆",
        MomentType::LiveBattle => "🏁",
    };

    output.push_str(&format!("# {} {}\n\n", type_emoji, moment.title));

    if let Some(ref subtitle) = moment.subtitle {
        output.push_str(&format!("*{}*\n\n", subtitle));
    }

    output.push_str(&format!(
        "**Type:** {}\n",
        moment.moment_type.display_name()
    ));
    output.push_str(&format!("**Score:** {}\n", moment.score));
    output.push_str(&format!(
        "**Agents involved:** {}\n",
        moment.agent_ids.len()
    ));
    output.push_str(&format!(
        "**Created:** {}\n\n",
        moment.created_at.format("%Y-%m-%d %H:%M UTC")
    ));

    if moment.promoted {
        output.push_str("⭐ *Staff Pick*\n\n");
    }

    output.push_str("---\n");
    output.push_str(&format!("React: `react <emoji> shame-{}`\n", moment.id));
    output.push_str(&format!("Comment: `comment shame-{} <text>`\n", moment.id));

    output
}
