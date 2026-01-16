//! Cross-platform path utilities
//!
//! Windows paths use backslashes (`\`) while internal instance paths use forward slashes (`/`).
//! These utilities ensure consistent path normalization across platforms.

use std::path::{Path, PathBuf};

/// Normalize path to forward slashes
#[inline]
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Convert PathBuf to normalized string
#[inline]
pub fn path_to_string(path: &Path) -> String {
    normalize_path(&path.to_string_lossy())
}

/// Append suffix to path, return normalized string
#[inline]
pub fn path_with_suffix(path: &Path, suffix: &str) -> String {
    format!("{}{}", path_to_string(path), suffix)
}

/// Create PathBuf from path + suffix
#[inline]
pub fn pathbuf_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(path_with_suffix(path, suffix))
}

/// Sanitize filename for Windows compatibility
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("foo\\bar\\baz"), "foo/bar/baz");
        assert_eq!(normalize_path("foo/bar/baz"), "foo/bar/baz");
        assert_eq!(normalize_path(""), "");
        assert_eq!(normalize_path("C:\\Users\\test\\project"), "C:/Users/test/project");
    }

    #[test]
    fn test_path_to_string() {
        let path = PathBuf::from("ServerScriptService").join("MyScript");
        let result = path_to_string(&path);
        assert!(!result.contains('\\'));
        assert!(result.contains('/') || result == "ServerScriptService/MyScript" || result.ends_with("MyScript"));
    }

    #[test]
    fn test_path_with_suffix() {
        let path = PathBuf::from("ServerScriptService").join("MyScript");
        let result = path_with_suffix(&path, ".server.luau");
        assert!(!result.contains('\\'));
        assert!(result.ends_with(".server.luau"));
    }

    #[test]
    fn test_pathbuf_with_suffix() {
        let path = PathBuf::from("Workspace").join("Part");
        let result = pathbuf_with_suffix(&path, ".rbxjson");
        let result_str = result.to_string_lossy();
        assert!(result_str.ends_with(".rbxjson"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("normal_name"), "normal_name");
        assert_eq!(sanitize_filename("file<>:name"), "file___name");
        assert_eq!(sanitize_filename("question?mark"), "question_mark");
        assert_eq!(sanitize_filename("star*name"), "star_name");
    }
}
