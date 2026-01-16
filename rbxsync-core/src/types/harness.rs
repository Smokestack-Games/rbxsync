//! Harness System Types
//!
//! Types for multi-session AI game development, enabling structured
//! feature tracking and session management across Claude sessions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// The main game definition file (game.yaml)
/// Describes the game being developed across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDefinition {
    /// Unique identifier for this game project
    #[serde(default = "generate_uuid")]
    pub id: String,

    /// Human-readable name of the game
    pub name: String,

    /// Brief description of the game
    #[serde(default)]
    pub description: String,

    /// Game genre (e.g., "RPG", "Simulator", "Obby")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,

    /// Target audience or player base
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_audience: Option<String>,

    /// High-level design goals for the game
    #[serde(default)]
    pub design_goals: Vec<String>,

    /// Technical constraints or requirements
    #[serde(default)]
    pub constraints: Vec<String>,

    /// Reference materials (links, docs, inspiration)
    #[serde(default)]
    pub references: Vec<String>,

    /// Creation timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// Last modified timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

impl Default for GameDefinition {
    fn default() -> Self {
        Self {
            id: generate_uuid(),
            name: "Untitled Game".to_string(),
            description: String::new(),
            genre: None,
            target_audience: None,
            design_goals: Vec::new(),
            constraints: Vec::new(),
            references: Vec::new(),
            created_at: None,
            updated_at: None,
        }
    }
}

/// Status of a feature in development
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    /// Feature is planned but not started
    #[default]
    Planned,

    /// Feature is currently being worked on
    InProgress,

    /// Feature implementation is complete
    Completed,

    /// Feature is blocked by dependencies or issues
    Blocked,

    /// Feature has been cancelled or removed from scope
    Cancelled,
}

impl std::fmt::Display for FeatureStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeatureStatus::Planned => write!(f, "planned"),
            FeatureStatus::InProgress => write!(f, "in_progress"),
            FeatureStatus::Completed => write!(f, "completed"),
            FeatureStatus::Blocked => write!(f, "blocked"),
            FeatureStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Priority level for a feature
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum FeaturePriority {
    /// Critical feature, must be implemented
    Critical,

    /// High priority feature
    High,

    /// Normal priority
    #[default]
    Medium,

    /// Low priority, nice to have
    Low,
}

/// A feature being developed in the game
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    /// Unique identifier for this feature
    #[serde(default = "generate_uuid")]
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Detailed description of the feature
    #[serde(default)]
    pub description: String,

    /// Current status
    #[serde(default)]
    pub status: FeatureStatus,

    /// Priority level
    #[serde(default)]
    pub priority: FeaturePriority,

    /// IDs of features this depends on
    #[serde(default)]
    pub dependencies: Vec<String>,

    /// Tags for categorization (e.g., "ui", "gameplay", "backend")
    #[serde(default)]
    pub tags: Vec<String>,

    /// Acceptance criteria - conditions for completion
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,

    /// Implementation notes from sessions
    #[serde(default)]
    pub notes: Vec<String>,

    /// Files modified or created for this feature
    #[serde(default)]
    pub affected_files: Vec<String>,

    /// Session IDs that worked on this feature
    #[serde(default)]
    pub session_ids: Vec<String>,

    /// Estimated complexity (1-5 scale, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<u8>,

    /// Reason for being blocked (if status is Blocked)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,

    /// Creation timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// Last modified timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,

    /// Completion timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

impl Default for Feature {
    fn default() -> Self {
        Self {
            id: generate_uuid(),
            name: "Untitled Feature".to_string(),
            description: String::new(),
            status: FeatureStatus::default(),
            priority: FeaturePriority::default(),
            dependencies: Vec::new(),
            tags: Vec::new(),
            acceptance_criteria: Vec::new(),
            notes: Vec::new(),
            affected_files: Vec::new(),
            session_ids: Vec::new(),
            complexity: None,
            blocked_reason: None,
            created_at: None,
            updated_at: None,
            completed_at: None,
        }
    }
}

/// Log entry for a development session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLogEntry {
    /// Timestamp of this entry (ISO 8601)
    pub timestamp: String,

    /// Type of entry (e.g., "start", "feature_update", "note", "end")
    pub entry_type: String,

    /// Message or description
    pub message: String,

    /// Related feature ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_id: Option<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// A development session log
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLog {
    /// Unique identifier for this session
    #[serde(default = "generate_uuid")]
    pub id: String,

    /// Session start timestamp (ISO 8601)
    pub started_at: String,

    /// Session end timestamp (ISO 8601, None if session is active)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,

    /// Brief summary of what was accomplished
    #[serde(default)]
    pub summary: String,

    /// Features worked on during this session
    #[serde(default)]
    pub features_worked_on: Vec<String>,

    /// Files modified during this session
    #[serde(default)]
    pub files_modified: Vec<String>,

    /// Log entries during the session
    #[serde(default)]
    pub entries: Vec<SessionLogEntry>,

    /// Notes or observations for future sessions
    #[serde(default)]
    pub handoff_notes: Vec<String>,

    /// Issues or blockers encountered
    #[serde(default)]
    pub issues_encountered: Vec<String>,
}

impl Default for SessionLog {
    fn default() -> Self {
        Self {
            id: generate_uuid(),
            started_at: chrono_now(),
            ended_at: None,
            summary: String::new(),
            features_worked_on: Vec::new(),
            files_modified: Vec::new(),
            entries: Vec::new(),
            handoff_notes: Vec::new(),
            issues_encountered: Vec::new(),
        }
    }
}

/// Get current timestamp in ISO 8601 format
fn chrono_now() -> String {
    // Simple timestamp without chrono dependency
    // Format: 2024-01-15T10:30:00Z
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Convert to rough date/time (simplified, not accounting for leap years perfectly)
    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    // Calculate year, month, day from days since epoch (1970-01-01)
    let mut year = 1970;
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

/// Complete harness state for a project
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessState {
    /// The game definition
    pub game: GameDefinition,

    /// All features (keyed by feature ID)
    #[serde(default)]
    pub features: HashMap<String, Feature>,

    /// Current active session ID (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_session_id: Option<String>,

    /// Path to the harness directory
    #[serde(skip)]
    pub harness_dir: PathBuf,
}

impl HarnessState {
    /// Create a new harness state for a project
    pub fn new(harness_dir: PathBuf, game_name: &str) -> Self {
        Self {
            game: GameDefinition {
                name: game_name.to_string(),
                created_at: Some(chrono_now()),
                ..Default::default()
            },
            features: HashMap::new(),
            active_session_id: None,
            harness_dir,
        }
    }

    /// Get the path to game.yaml
    pub fn game_path(&self) -> PathBuf {
        self.harness_dir.join("game.yaml")
    }

    /// Get the path to features.yaml
    pub fn features_path(&self) -> PathBuf {
        self.harness_dir.join("features.yaml")
    }

    /// Get the sessions directory path
    pub fn sessions_dir(&self) -> PathBuf {
        self.harness_dir.join("sessions")
    }

    /// Get the path to a specific session log
    pub fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir().join(format!("{}.yaml", session_id))
    }

    /// Add a new feature
    pub fn add_feature(&mut self, feature: Feature) {
        self.features.insert(feature.id.clone(), feature);
    }

    /// Get a feature by ID
    pub fn get_feature(&self, id: &str) -> Option<&Feature> {
        self.features.get(id)
    }

    /// Get a mutable feature by ID
    pub fn get_feature_mut(&mut self, id: &str) -> Option<&mut Feature> {
        self.features.get_mut(id)
    }

    /// Update a feature's status
    pub fn update_feature_status(&mut self, id: &str, status: FeatureStatus) -> bool {
        if let Some(feature) = self.features.get_mut(id) {
            feature.status = status;
            feature.updated_at = Some(chrono_now());
            if status == FeatureStatus::Completed {
                feature.completed_at = Some(chrono_now());
            }
            true
        } else {
            false
        }
    }

    /// Get features by status
    pub fn features_by_status(&self, status: FeatureStatus) -> Vec<&Feature> {
        self.features
            .values()
            .filter(|f| f.status == status)
            .collect()
    }

    /// Get features by tag
    pub fn features_by_tag(&self, tag: &str) -> Vec<&Feature> {
        self.features
            .values()
            .filter(|f| f.tags.contains(&tag.to_string()))
            .collect()
    }
}

/// Features file structure (features.yaml)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeaturesFile {
    /// List of features
    pub features: Vec<Feature>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_definition_default() {
        let game = GameDefinition::default();
        assert!(!game.id.is_empty());
        assert_eq!(game.name, "Untitled Game");
    }

    #[test]
    fn test_feature_status_display() {
        assert_eq!(FeatureStatus::Planned.to_string(), "planned");
        assert_eq!(FeatureStatus::InProgress.to_string(), "in_progress");
        assert_eq!(FeatureStatus::Completed.to_string(), "completed");
    }

    #[test]
    fn test_harness_state_new() {
        let state = HarnessState::new(PathBuf::from("/test/harness"), "Test Game");
        assert_eq!(state.game.name, "Test Game");
        assert!(state.game.created_at.is_some());
    }

    #[test]
    fn test_feature_operations() {
        let mut state = HarnessState::new(PathBuf::from("/test"), "Test");

        let feature = Feature {
            id: "feat-1".to_string(),
            name: "Test Feature".to_string(),
            status: FeatureStatus::Planned,
            ..Default::default()
        };

        state.add_feature(feature);
        assert!(state.get_feature("feat-1").is_some());

        assert!(state.update_feature_status("feat-1", FeatureStatus::Completed));
        let f = state.get_feature("feat-1").unwrap();
        assert_eq!(f.status, FeatureStatus::Completed);
        assert!(f.completed_at.is_some());
    }

    #[test]
    fn test_chrono_now_format() {
        let ts = chrono_now();
        // Should match ISO 8601 format: YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20);
    }
}
