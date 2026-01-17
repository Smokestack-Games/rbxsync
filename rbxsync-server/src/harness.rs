//! Harness System HTTP Handlers
//!
//! Endpoints for multi-session AI game development tracking.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rbxsync_core::{
    Feature, FeaturePriority, FeatureStatus, FeaturesFile, GameDefinition,
    SessionLog, SessionLogEntry,
};
use serde::{Deserialize, Serialize};

// Embed templates at compile time
mod templates {
    pub const TYCOON: &str = include_str!("harness/templates/tycoon.yaml");
    pub const OBBY: &str = include_str!("harness/templates/obby.yaml");
    pub const SIMULATOR: &str = include_str!("harness/templates/simulator.yaml");
    pub const RPG: &str = include_str!("harness/templates/rpg.yaml");
    pub const HORROR: &str = include_str!("harness/templates/horror.yaml");
}

/// Template definition structure matching the YAML format
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameTemplate {
    genre: String,
    description: String,
    features: Vec<TemplateFeature>,
}

/// Feature definition within a template
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemplateFeature {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    priority: TemplatePriority,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default, rename = "acceptanceCriteria")]
    acceptance_criteria: Vec<String>,
    #[serde(default)]
    complexity: Option<u8>,
}

/// Priority level in template (matches core enum but for deserialization)
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TemplatePriority {
    Critical,
    High,
    #[default]
    Medium,
    Low,
}

impl From<TemplatePriority> for FeaturePriority {
    fn from(p: TemplatePriority) -> Self {
        match p {
            TemplatePriority::Critical => FeaturePriority::Critical,
            TemplatePriority::High => FeaturePriority::High,
            TemplatePriority::Medium => FeaturePriority::Medium,
            TemplatePriority::Low => FeaturePriority::Low,
        }
    }
}

/// Get template content by name
fn get_template(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "tycoon" => Some(templates::TYCOON),
        "obby" => Some(templates::OBBY),
        "simulator" => Some(templates::SIMULATOR),
        "rpg" => Some(templates::RPG),
        "horror" => Some(templates::HORROR),
        _ => None,
    }
}

/// List available template names
pub fn available_templates() -> Vec<&'static str> {
    vec!["tycoon", "obby", "simulator", "rpg", "horror"]
}

use crate::AppState;

/// Default harness directory name
const HARNESS_DIR: &str = ".rbxsync/harness";

/// Get the harness directory path for a project
fn get_harness_dir(project_dir: &str) -> PathBuf {
    PathBuf::from(project_dir).join(HARNESS_DIR)
}

/// Request to initialize a harness for a project
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessInitRequest {
    /// Project directory path
    pub project_dir: String,

    /// Game name
    pub game_name: String,

    /// Optional game description
    #[serde(default)]
    pub description: Option<String>,

    /// Optional game genre
    #[serde(default)]
    pub genre: Option<String>,

    /// Optional template to initialize with (tycoon, obby, simulator, rpg, horror)
    #[serde(default)]
    pub template: Option<String>,
}

/// Response from harness init
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessInitResponse {
    pub success: bool,
    pub message: String,
    pub harness_dir: String,
    pub game_id: Option<String>,
    /// Template that was applied (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_applied: Option<String>,
    /// Number of features added from template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features_added: Option<usize>,
}

/// Initialize harness directory structure for a project
pub async fn handle_harness_init(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<HarnessInitRequest>,
) -> impl IntoResponse {
    tracing::info!("Initializing harness for project: {}", req.project_dir);

    let harness_dir = get_harness_dir(&req.project_dir);
    let sessions_dir = harness_dir.join("sessions");

    // Create directory structure
    if let Err(e) = std::fs::create_dir_all(&sessions_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(HarnessInitResponse {
                success: false,
                message: format!("Failed to create harness directories: {}", e),
                harness_dir: harness_dir.to_string_lossy().to_string(),
                game_id: None,
                template_applied: None,
                features_added: None,
            }),
        );
    }

    // Create game definition
    let mut game = GameDefinition {
        name: req.game_name,
        created_at: Some(current_timestamp()),
        ..Default::default()
    };

    if let Some(desc) = req.description {
        game.description = desc;
    }
    if let Some(genre) = req.genre {
        game.genre = Some(genre);
    }

    let game_id = game.id.clone();

    // Write game.yaml
    let game_path = harness_dir.join("game.yaml");
    match serde_yaml::to_string(&game) {
        Ok(yaml) => {
            if let Err(e) = std::fs::write(&game_path, yaml) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(HarnessInitResponse {
                        success: false,
                        message: format!("Failed to write game.yaml: {}", e),
                        harness_dir: harness_dir.to_string_lossy().to_string(),
                        game_id: None,
                        template_applied: None,
                        features_added: None,
                    }),
                );
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(HarnessInitResponse {
                    success: false,
                    message: format!("Failed to serialize game definition: {}", e),
                    harness_dir: harness_dir.to_string_lossy().to_string(),
                    game_id: None,
                    template_applied: None,
                    features_added: None,
                }),
            );
        }
    }

    // Load template features if specified
    let (features, template_applied, features_count) = if let Some(ref template_name) = req.template {
        match get_template(template_name) {
            Some(template_content) => {
                match serde_yaml::from_str::<GameTemplate>(template_content) {
                    Ok(template) => {
                        let timestamp = current_timestamp();
                        let features: Vec<Feature> = template.features.into_iter().map(|tf| {
                            Feature {
                                name: tf.name,
                                description: tf.description,
                                priority: tf.priority.into(),
                                tags: tf.tags,
                                acceptance_criteria: tf.acceptance_criteria,
                                complexity: tf.complexity,
                                created_at: Some(timestamp.clone()),
                                // Dependencies stored as names in template notes
                                notes: if tf.dependencies.is_empty() {
                                    vec![]
                                } else {
                                    vec![format!("Depends on: {}", tf.dependencies.join(", "))]
                                },
                                ..Default::default()
                            }
                        }).collect();
                        let count = features.len();
                        (FeaturesFile { features }, Some(template_name.clone()), Some(count))
                    }
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(HarnessInitResponse {
                                success: false,
                                message: format!("Failed to parse template '{}': {}", template_name, e),
                                harness_dir: harness_dir.to_string_lossy().to_string(),
                                game_id: Some(game_id),
                                template_applied: None,
                                features_added: None,
                            }),
                        );
                    }
                }
            }
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(HarnessInitResponse {
                        success: false,
                        message: format!(
                            "Unknown template '{}'. Available: {}",
                            template_name,
                            available_templates().join(", ")
                        ),
                        harness_dir: harness_dir.to_string_lossy().to_string(),
                        game_id: Some(game_id),
                        template_applied: None,
                        features_added: None,
                    }),
                );
            }
        }
    } else {
        (FeaturesFile { features: vec![] }, None, None)
    };

    // Write features.yaml
    let features_path = harness_dir.join("features.yaml");
    match serde_yaml::to_string(&features) {
        Ok(yaml) => {
            if let Err(e) = std::fs::write(&features_path, yaml) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(HarnessInitResponse {
                        success: false,
                        message: format!("Failed to write features.yaml: {}", e),
                        harness_dir: harness_dir.to_string_lossy().to_string(),
                        game_id: Some(game_id),
                        template_applied: None,
                        features_added: None,
                    }),
                );
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(HarnessInitResponse {
                    success: false,
                    message: format!("Failed to serialize features: {}", e),
                    harness_dir: harness_dir.to_string_lossy().to_string(),
                    game_id: Some(game_id),
                    template_applied: None,
                    features_added: None,
                }),
            );
        }
    }

    let message = match &template_applied {
        Some(t) => format!(
            "Harness initialized with '{}' template ({} features)",
            t,
            features_count.unwrap_or(0)
        ),
        None => "Harness initialized successfully".to_string(),
    };

    tracing::info!(
        "Harness initialized at: {} (template: {:?})",
        harness_dir.display(),
        template_applied
    );

    (
        StatusCode::OK,
        Json(HarnessInitResponse {
            success: true,
            message,
            harness_dir: harness_dir.to_string_lossy().to_string(),
            game_id: Some(game_id),
            template_applied,
            features_added: features_count,
        }),
    )
}

/// Request to start a new session
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartRequest {
    /// Project directory path
    pub project_dir: String,

    /// Optional initial summary/goals for the session
    #[serde(default)]
    pub initial_goals: Option<String>,
}

/// Response from session start
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub session_path: Option<String>,
}

/// Start a new development session
pub async fn handle_session_start(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<SessionStartRequest>,
) -> impl IntoResponse {
    tracing::info!("Starting new session for project: {}", req.project_dir);

    let harness_dir = get_harness_dir(&req.project_dir);
    let sessions_dir = harness_dir.join("sessions");

    // Verify harness exists
    if !harness_dir.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(SessionStartResponse {
                success: false,
                message: "Harness not initialized. Call /harness/init first.".to_string(),
                session_id: None,
                session_path: None,
            }),
        );
    }

    // Create session log
    let mut session = SessionLog::default();
    if let Some(goals) = req.initial_goals {
        session.entries.push(SessionLogEntry {
            timestamp: current_timestamp(),
            entry_type: "start".to_string(),
            message: format!("Session started. Goals: {}", goals),
            feature_id: None,
            metadata: Default::default(),
        });
    } else {
        session.entries.push(SessionLogEntry {
            timestamp: current_timestamp(),
            entry_type: "start".to_string(),
            message: "Session started".to_string(),
            feature_id: None,
            metadata: Default::default(),
        });
    }

    let session_id = session.id.clone();
    let session_path = sessions_dir.join(format!("{}.yaml", session_id));

    // Write session file
    match serde_yaml::to_string(&session) {
        Ok(yaml) => {
            if let Err(e) = std::fs::write(&session_path, yaml) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SessionStartResponse {
                        success: false,
                        message: format!("Failed to write session file: {}", e),
                        session_id: None,
                        session_path: None,
                    }),
                );
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SessionStartResponse {
                    success: false,
                    message: format!("Failed to serialize session: {}", e),
                    session_id: None,
                    session_path: None,
                }),
            );
        }
    }

    tracing::info!("Session started: {}", session_id);

    (
        StatusCode::OK,
        Json(SessionStartResponse {
            success: true,
            message: "Session started successfully".to_string(),
            session_id: Some(session_id),
            session_path: Some(session_path.to_string_lossy().to_string()),
        }),
    )
}

/// Request to end a session
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEndRequest {
    /// Project directory path
    pub project_dir: String,

    /// Session ID to end
    pub session_id: String,

    /// Summary of what was accomplished
    #[serde(default)]
    pub summary: Option<String>,

    /// Handoff notes for future sessions
    #[serde(default)]
    pub handoff_notes: Option<Vec<String>>,
}

/// Response from session end
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEndResponse {
    pub success: bool,
    pub message: String,
}

/// End a development session
pub async fn handle_session_end(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<SessionEndRequest>,
) -> impl IntoResponse {
    tracing::info!("Ending session {} for project: {}", req.session_id, req.project_dir);

    let harness_dir = get_harness_dir(&req.project_dir);
    let session_path = harness_dir.join("sessions").join(format!("{}.yaml", req.session_id));

    // Read existing session
    let session_content = match std::fs::read_to_string(&session_path) {
        Ok(content) => content,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(SessionEndResponse {
                    success: false,
                    message: format!("Session not found: {}", e),
                }),
            );
        }
    };

    let mut session: SessionLog = match serde_yaml::from_str(&session_content) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SessionEndResponse {
                    success: false,
                    message: format!("Failed to parse session file: {}", e),
                }),
            );
        }
    };

    // Update session
    session.ended_at = Some(current_timestamp());
    if let Some(summary) = req.summary {
        session.summary = summary;
    }
    if let Some(notes) = req.handoff_notes {
        session.handoff_notes = notes;
    }
    session.entries.push(SessionLogEntry {
        timestamp: current_timestamp(),
        entry_type: "end".to_string(),
        message: "Session ended".to_string(),
        feature_id: None,
        metadata: Default::default(),
    });

    // Write updated session
    match serde_yaml::to_string(&session) {
        Ok(yaml) => {
            if let Err(e) = std::fs::write(&session_path, yaml) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SessionEndResponse {
                        success: false,
                        message: format!("Failed to update session file: {}", e),
                    }),
                );
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SessionEndResponse {
                    success: false,
                    message: format!("Failed to serialize session: {}", e),
                }),
            );
        }
    }

    tracing::info!("Session ended: {}", req.session_id);

    (
        StatusCode::OK,
        Json(SessionEndResponse {
            success: true,
            message: "Session ended successfully".to_string(),
        }),
    )
}

/// Request to update a feature
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureUpdateRequest {
    /// Project directory path
    pub project_dir: String,

    /// Feature ID (if updating existing feature)
    #[serde(default)]
    pub feature_id: Option<String>,

    /// Feature name (required for new features)
    #[serde(default)]
    pub name: Option<String>,

    /// Feature description
    #[serde(default)]
    pub description: Option<String>,

    /// New status
    #[serde(default)]
    pub status: Option<FeatureStatus>,

    /// Priority
    #[serde(default)]
    pub priority: Option<FeaturePriority>,

    /// Tags to add
    #[serde(default)]
    pub tags: Option<Vec<String>>,

    /// Acceptance criteria
    #[serde(default)]
    pub acceptance_criteria: Option<Vec<String>>,

    /// Note to add
    #[serde(default)]
    pub add_note: Option<String>,

    /// Files affected
    #[serde(default)]
    pub affected_files: Option<Vec<String>>,

    /// Session ID working on this feature
    #[serde(default)]
    pub session_id: Option<String>,

    /// Blocked reason (if setting status to blocked)
    #[serde(default)]
    pub blocked_reason: Option<String>,

    /// Dependencies (feature IDs)
    #[serde(default)]
    pub dependencies: Option<Vec<String>>,

    /// Complexity (1-5)
    #[serde(default)]
    pub complexity: Option<u8>,
}

/// Response from feature update
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureUpdateResponse {
    pub success: bool,
    pub message: String,
    pub feature_id: Option<String>,
}

/// Update or create a feature
pub async fn handle_feature_update(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<FeatureUpdateRequest>,
) -> impl IntoResponse {
    tracing::info!("Feature update for project: {}", req.project_dir);

    let harness_dir = get_harness_dir(&req.project_dir);
    let features_path = harness_dir.join("features.yaml");

    // Verify harness exists
    if !harness_dir.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(FeatureUpdateResponse {
                success: false,
                message: "Harness not initialized. Call /harness/init first.".to_string(),
                feature_id: None,
            }),
        );
    }

    // Read existing features
    let mut features_file: FeaturesFile = if features_path.exists() {
        match std::fs::read_to_string(&features_path) {
            Ok(content) => match serde_yaml::from_str(&content) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("Failed to parse features.yaml: {}, starting fresh", e);
                    FeaturesFile { features: vec![] }
                }
            },
            Err(_) => FeaturesFile { features: vec![] },
        }
    } else {
        FeaturesFile { features: vec![] }
    };

    let feature_id: String;
    let is_new: bool;

    if let Some(existing_id) = req.feature_id {
        // Update existing feature
        feature_id = existing_id.clone();
        is_new = false;

        if let Some(feature) = features_file.features.iter_mut().find(|f| f.id == existing_id) {
            // Apply updates
            if let Some(name) = req.name {
                feature.name = name;
            }
            if let Some(desc) = req.description {
                feature.description = desc;
            }
            if let Some(status) = req.status {
                feature.status = status;
                if status == FeatureStatus::Completed {
                    feature.completed_at = Some(current_timestamp());
                }
            }
            if let Some(priority) = req.priority {
                feature.priority = priority;
            }
            if let Some(tags) = req.tags {
                feature.tags = tags;
            }
            if let Some(criteria) = req.acceptance_criteria {
                feature.acceptance_criteria = criteria;
            }
            if let Some(note) = req.add_note {
                feature.notes.push(note);
            }
            if let Some(files) = req.affected_files {
                for file in files {
                    if !feature.affected_files.contains(&file) {
                        feature.affected_files.push(file);
                    }
                }
            }
            if let Some(session_id) = req.session_id {
                if !feature.session_ids.contains(&session_id) {
                    feature.session_ids.push(session_id);
                }
            }
            if let Some(reason) = req.blocked_reason {
                feature.blocked_reason = Some(reason);
            }
            if let Some(deps) = req.dependencies {
                feature.dependencies = deps;
            }
            if let Some(complexity) = req.complexity {
                feature.complexity = Some(complexity);
            }
            feature.updated_at = Some(current_timestamp());
        } else {
            return (
                StatusCode::NOT_FOUND,
                Json(FeatureUpdateResponse {
                    success: false,
                    message: format!("Feature not found: {}", existing_id),
                    feature_id: None,
                }),
            );
        }
    } else {
        // Create new feature
        let name = match req.name {
            Some(n) => n,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(FeatureUpdateResponse {
                        success: false,
                        message: "Name is required for new features".to_string(),
                        feature_id: None,
                    }),
                );
            }
        };

        let mut feature = Feature {
            name,
            created_at: Some(current_timestamp()),
            ..Default::default()
        };

        feature_id = feature.id.clone();
        is_new = true;

        if let Some(desc) = req.description {
            feature.description = desc;
        }
        if let Some(status) = req.status {
            feature.status = status;
        }
        if let Some(priority) = req.priority {
            feature.priority = priority;
        }
        if let Some(tags) = req.tags {
            feature.tags = tags;
        }
        if let Some(criteria) = req.acceptance_criteria {
            feature.acceptance_criteria = criteria;
        }
        if let Some(note) = req.add_note {
            feature.notes.push(note);
        }
        if let Some(files) = req.affected_files {
            feature.affected_files = files;
        }
        if let Some(session_id) = req.session_id {
            feature.session_ids.push(session_id);
        }
        if let Some(deps) = req.dependencies {
            feature.dependencies = deps;
        }
        if let Some(complexity) = req.complexity {
            feature.complexity = Some(complexity);
        }

        features_file.features.push(feature);
    }

    // Write updated features
    match serde_yaml::to_string(&features_file) {
        Ok(yaml) => {
            if let Err(e) = std::fs::write(&features_path, yaml) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(FeatureUpdateResponse {
                        success: false,
                        message: format!("Failed to write features file: {}", e),
                        feature_id: Some(feature_id),
                    }),
                );
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(FeatureUpdateResponse {
                    success: false,
                    message: format!("Failed to serialize features: {}", e),
                    feature_id: Some(feature_id),
                }),
            );
        }
    }

    let action = if is_new { "created" } else { "updated" };
    tracing::info!("Feature {}: {}", action, feature_id);

    (
        StatusCode::OK,
        Json(FeatureUpdateResponse {
            success: true,
            message: format!("Feature {} successfully", action),
            feature_id: Some(feature_id),
        }),
    )
}

/// Request for harness status
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessStatusRequest {
    /// Project directory path
    pub project_dir: String,
}

/// Response with harness status
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessStatusResponse {
    pub success: bool,
    pub initialized: bool,
    pub game: Option<GameDefinition>,
    pub features: Vec<Feature>,
    pub feature_summary: FeatureSummary,
    pub recent_sessions: Vec<SessionSummary>,
}

/// Summary of feature statuses
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSummary {
    pub total: usize,
    pub planned: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub blocked: usize,
    pub cancelled: usize,
}

/// Brief session summary
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub summary: String,
    pub features_count: usize,
}

/// Get harness status for a project
pub async fn handle_harness_status(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<HarnessStatusRequest>,
) -> impl IntoResponse {
    tracing::info!("Getting harness status for project: {}", req.project_dir);

    let harness_dir = get_harness_dir(&req.project_dir);

    if !harness_dir.exists() {
        return Json(HarnessStatusResponse {
            success: true,
            initialized: false,
            game: None,
            features: vec![],
            feature_summary: FeatureSummary::default(),
            recent_sessions: vec![],
        });
    }

    // Read game definition
    let game_path = harness_dir.join("game.yaml");
    let game: Option<GameDefinition> = if game_path.exists() {
        std::fs::read_to_string(&game_path)
            .ok()
            .and_then(|content| serde_yaml::from_str(&content).ok())
    } else {
        None
    };

    // Read features
    let features_path = harness_dir.join("features.yaml");
    let features_file: FeaturesFile = if features_path.exists() {
        std::fs::read_to_string(&features_path)
            .ok()
            .and_then(|content| serde_yaml::from_str(&content).ok())
            .unwrap_or(FeaturesFile { features: vec![] })
    } else {
        FeaturesFile { features: vec![] }
    };

    // Calculate feature summary
    let mut summary = FeatureSummary {
        total: features_file.features.len(),
        ..Default::default()
    };
    for feature in &features_file.features {
        match feature.status {
            FeatureStatus::Planned => summary.planned += 1,
            FeatureStatus::InProgress => summary.in_progress += 1,
            FeatureStatus::Completed => summary.completed += 1,
            FeatureStatus::Blocked => summary.blocked += 1,
            FeatureStatus::Cancelled => summary.cancelled += 1,
        }
    }

    // Read recent sessions (up to 5)
    let sessions_dir = harness_dir.join("sessions");
    let mut recent_sessions = vec![];
    if sessions_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
            let mut session_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|ext| ext == "yaml").unwrap_or(false))
                .collect();

            // Sort by modification time (newest first)
            session_files.sort_by(|a, b| {
                let a_time = a.metadata().and_then(|m| m.modified()).ok();
                let b_time = b.metadata().and_then(|m| m.modified()).ok();
                b_time.cmp(&a_time)
            });

            for entry in session_files.into_iter().take(5) {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if let Ok(session) = serde_yaml::from_str::<SessionLog>(&content) {
                        recent_sessions.push(SessionSummary {
                            id: session.id,
                            started_at: session.started_at,
                            ended_at: session.ended_at,
                            summary: session.summary,
                            features_count: session.features_worked_on.len(),
                        });
                    }
                }
            }
        }
    }

    Json(HarnessStatusResponse {
        success: true,
        initialized: true,
        game,
        features: features_file.features,
        feature_summary: summary,
        recent_sessions,
    })
}

/// Get current timestamp in ISO 8601 format
fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    let mut year = 1970u64;
    let mut remaining_days = days;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let mut month = 1;
    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    for days_in_month in days_in_months.iter() {
        if remaining_days < *days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }

    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_timestamp_format() {
        let ts = current_timestamp();
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20);
    }

    #[test]
    fn test_get_harness_dir() {
        let dir = get_harness_dir("/project");
        assert_eq!(dir, PathBuf::from("/project/.rbxsync/harness"));
    }
}
