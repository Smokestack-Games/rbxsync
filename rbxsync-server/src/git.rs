//! Git integration module
//!
//! Provides git commands for the RbxSync plugin to display status,
//! commit changes, and view history.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// Git repository status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: String,
    pub is_dirty: bool,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
    pub ahead: usize,
    pub behind: usize,
}

/// Git commit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

/// Get git status for a project directory
pub fn get_status(project_dir: &Path) -> Result<GitStatus, String> {
    // Check if directory is a git repo
    if !project_dir.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    // Get current branch
    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("Failed to get branch: {}", e))?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    // Get status
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("Failed to get status: {}", e))?;

    let status_str = String::from_utf8_lossy(&status_output.stdout);

    let mut staged_count = 0;
    let mut unstaged_count = 0;
    let mut untracked_count = 0;

    for line in status_str.lines() {
        if line.len() < 2 {
            continue;
        }
        let first = line.chars().next().unwrap_or(' ');
        let second = line.chars().nth(1).unwrap_or(' ');

        if line.starts_with("??") {
            untracked_count += 1;
        } else {
            // First character is index status, second is working tree status
            if first != ' ' && first != '?' {
                staged_count += 1;
            }
            if second != ' ' && second != '?' {
                unstaged_count += 1;
            }
        }
    }

    // Get ahead/behind count
    let mut ahead = 0;
    let mut behind = 0;

    let rev_output = Command::new("git")
        .args(["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
        .current_dir(project_dir)
        .output();

    if let Ok(output) = rev_output {
        if output.status.success() {
            let counts = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = counts.trim().split('\t').collect();
            if parts.len() == 2 {
                ahead = parts[0].parse().unwrap_or(0);
                behind = parts[1].parse().unwrap_or(0);
            }
        }
    }

    let is_dirty = staged_count > 0 || unstaged_count > 0 || untracked_count > 0;

    Ok(GitStatus {
        branch,
        is_dirty,
        staged_count,
        unstaged_count,
        untracked_count,
        ahead,
        behind,
    })
}

/// Get recent commit history
pub fn get_log(project_dir: &Path, limit: usize) -> Result<Vec<GitCommit>, String> {
    // Check if directory is a git repo
    if !project_dir.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    let output = Command::new("git")
        .args([
            "log",
            &format!("-{}", limit),
            "--pretty=format:%h|%s|%an|%ar",
        ])
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("Failed to get log: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let log_str = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<GitCommit> = log_str
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                Some(GitCommit {
                    hash: parts[0].to_string(),
                    message: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(commits)
}

/// Commit all changes with a message
pub fn commit(project_dir: &Path, message: &str, add_all: bool) -> Result<String, String> {
    // Check if directory is a git repo
    if !project_dir.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    // Add all changes if requested
    if add_all {
        let add_output = Command::new("git")
            .args(["add", "-A"])
            .current_dir(project_dir)
            .output()
            .map_err(|e| format!("Failed to stage changes: {}", e))?;

        if !add_output.status.success() {
            return Err(String::from_utf8_lossy(&add_output.stderr).to_string());
        }
    }

    // Commit
    let commit_output = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("Failed to commit: {}", e))?;

    if commit_output.status.success() {
        Ok(String::from_utf8_lossy(&commit_output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&commit_output.stderr).to_string())
    }
}

/// Initialize a new git repository
pub fn init(project_dir: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["init"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("Failed to init: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
