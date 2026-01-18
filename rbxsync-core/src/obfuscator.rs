//! Luau Obfuscator
//!
//! Build-time obfuscation for Luau source files to evade simple string detection.
//! This module transforms sensitive strings using hex encoding which Luau parses
//! at compile time (not runtime), so functionality is preserved.
//!
//! Techniques used:
//! - Hex-encoded string literals ("\x67\x65\x74" -> "get" at parse time)
//! - Debug statement stripping
//! - Comment removal
//! - Variable prefix randomization

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rand::Rng;
use serde::Deserialize;

/// Configuration for the obfuscator loaded from obfuscate.toml
#[derive(Debug, Clone, Deserialize)]
pub struct ObfuscatorConfig {
    /// Strings to encode with hex escapes
    #[serde(default)]
    pub strings: StringConfig,
    /// Debug patterns to strip
    #[serde(default)]
    pub debug: DebugConfig,
    /// Minification options
    #[serde(default)]
    pub minify: MinifyConfig,
}

impl Default for ObfuscatorConfig {
    fn default() -> Self {
        Self {
            strings: StringConfig::default(),
            debug: DebugConfig::default(),
            minify: MinifyConfig::default(),
        }
    }
}

/// String encoding configuration
#[derive(Debug, Clone, Deserialize)]
pub struct StringConfig {
    /// Strings to encode as hex escape sequences
    #[serde(default = "default_encode_strings")]
    pub encode: Vec<String>,
}

impl Default for StringConfig {
    fn default() -> Self {
        Self {
            encode: default_encode_strings(),
        }
    }
}

fn default_encode_strings() -> Vec<String> {
    vec![
        // Sensitive API names that might trigger detection
        "getfenv".to_string(),
        "setfenv".to_string(),
        "loadstring".to_string(),
        "InsertService".to_string(),
        "LoadStringEnabled".to_string(),
        "LoadAsset".to_string(),
        // Common detection targets
        "HttpService".to_string(),
        "require".to_string(),
    ]
}

/// Debug stripping configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DebugConfig {
    /// Regex patterns for lines to remove entirely
    #[serde(default = "default_strip_patterns")]
    pub strip_patterns: Vec<String>,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            strip_patterns: default_strip_patterns(),
        }
    }
}

fn default_strip_patterns() -> Vec<String> {
    vec![
        // Debug print statements with specific prefixes
        r#"^\s*print\s*\(\s*"\[RbxSync"#.to_string(),
        r#"^\s*print\s*\(\s*"\[BotRunner"#.to_string(),
        r#"^\s*print\s*\(\s*"\[DEBUG"#.to_string(),
        r#"^\s*warn\s*\(\s*"\[RbxSync"#.to_string(),
    ]
}

/// Minification configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MinifyConfig {
    /// Remove single-line comments (-- comment)
    #[serde(default)]
    pub strip_comments: bool,
    /// Remove multi-line comments (--[[ comment ]])
    #[serde(default)]
    pub strip_block_comments: bool,
}

/// Result of obfuscating a file
#[derive(Debug, Clone)]
pub struct ObfuscationResult {
    /// The transformed source code
    pub source: String,
    /// Number of strings encoded
    pub strings_encoded: usize,
    /// Number of debug statements stripped
    pub debug_stripped: usize,
    /// Number of comments removed
    pub comments_removed: usize,
}

impl ObfuscationResult {
    pub fn total_transforms(&self) -> usize {
        self.strings_encoded + self.debug_stripped + self.comments_removed
    }
}

/// Obfuscator instance with configuration
pub struct Obfuscator {
    config: ObfuscatorConfig,
    /// Random prefix for _0x style variables (per file)
    var_prefix: String,
    /// Compiled regex patterns for debug stripping
    strip_regexes: Vec<regex::Regex>,
}

impl Obfuscator {
    /// Create a new obfuscator with the given configuration
    pub fn new(config: ObfuscatorConfig) -> Self {
        let strip_regexes = config
            .debug
            .strip_patterns
            .iter()
            .filter_map(|p| regex::Regex::new(p).ok())
            .collect();

        Self {
            config,
            var_prefix: generate_random_prefix(),
            strip_regexes,
        }
    }

    /// Create an obfuscator with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ObfuscatorConfig::default())
    }

    /// Load configuration from a TOML file
    pub fn from_config_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: ObfuscatorConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        Ok(Self::new(config))
    }

    /// Regenerate the random variable prefix (call once per file)
    pub fn regenerate_prefix(&mut self) {
        self.var_prefix = generate_random_prefix();
    }

    /// Obfuscate a Luau source string
    pub fn obfuscate(&self, source: &str) -> ObfuscationResult {
        let mut result = source.to_string();
        let mut strings_encoded = 0;
        let mut debug_stripped = 0;
        let mut comments_removed = 0;

        // 1. Strip debug statements (line by line)
        let lines: Vec<&str> = result.lines().collect();
        let mut filtered_lines = Vec::with_capacity(lines.len());
        for line in lines {
            let should_strip = self.strip_regexes.iter().any(|re| re.is_match(line));
            if should_strip {
                debug_stripped += 1;
            } else {
                filtered_lines.push(line);
            }
        }
        result = filtered_lines.join("\n");

        // 2. Strip comments if configured
        if self.config.minify.strip_block_comments {
            let (new_source, count) = strip_block_comments(&result);
            result = new_source;
            comments_removed += count;
        }
        if self.config.minify.strip_comments {
            let (new_source, count) = strip_line_comments(&result);
            result = new_source;
            comments_removed += count;
        }

        // 3. Encode sensitive strings with hex escapes
        for target in &self.config.strings.encode {
            let count = encode_string_occurrences(&mut result, target);
            strings_encoded += count;
        }

        // 4. Replace _0x prefixes with random prefix
        result = replace_hex_prefixes(&result, &self.var_prefix);

        ObfuscationResult {
            source: result,
            strings_encoded,
            debug_stripped,
            comments_removed,
        }
    }

    /// Obfuscate a Luau source file, returning the transformed content
    pub fn obfuscate_file(&self, path: &Path) -> Result<ObfuscationResult> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;
        Ok(self.obfuscate(&source))
    }
}

/// Generate a random 2-character prefix for variable names
fn generate_random_prefix() -> String {
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = ('a'..='z').collect();
    let c1 = chars[rng.gen_range(0..chars.len())];
    let c2 = chars[rng.gen_range(0..chars.len())];
    format!("_{}{}", c1, c2)
}

/// Convert a string to hex-escaped format
/// "get" -> "\x67\x65\x74"
fn to_hex_escaped(s: &str) -> String {
    s.bytes().map(|b| format!("\\x{:02x}", b)).collect()
}

/// Encode occurrences of a string literal with hex escapes
/// This transforms "getfenv" -> "\x67\x65\x74\x66\x65\x6e\x76"
/// Luau parses hex escapes at compile time, so this is transparent at runtime
fn encode_string_occurrences(source: &mut String, target: &str) -> usize {
    let mut count = 0;
    let hex_encoded = to_hex_escaped(target);

    // Encode double-quoted strings: "target" -> "\xNN\xNN..."
    let double_quoted = format!(r#""{}""#, target);
    if source.contains(&double_quoted) {
        count += source.matches(&double_quoted).count();
        *source = source.replace(&double_quoted, &format!(r#""{}""#, hex_encoded));
    }

    // Encode single-quoted strings: 'target' -> '\xNN\xNN...'
    let single_quoted = format!("'{}'", target);
    if source.contains(&single_quoted) {
        count += source.matches(&single_quoted).count();
        *source = source.replace(&single_quoted, &format!("'{}'", hex_encoded));
    }

    count
}

/// Strip single-line comments (-- comment) but preserve string contents
fn strip_line_comments(source: &str) -> (String, usize) {
    let mut result = String::with_capacity(source.len());
    let mut count = 0;

    for line in source.lines() {
        // Simple approach: find -- that's not inside a string
        // This is a basic implementation that handles most cases
        if let Some(stripped) = strip_comment_from_line(line) {
            if stripped.len() < line.len() {
                count += 1;
            }
            result.push_str(stripped.trim_end());
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }

    // Remove trailing newline if original didn't have one
    if !source.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    (result, count)
}

/// Strip comment from a single line, being careful about strings
fn strip_comment_from_line(line: &str) -> Option<&str> {
    // Skip block comment starters
    if line.contains("--[[") || line.contains("--[=[") {
        return Some(line);
    }

    let mut in_string = false;
    let mut string_char = '"';
    let chars: Vec<char> = line.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        // Track string state
        if !in_string && (c == '"' || c == '\'') {
            in_string = true;
            string_char = c;
        } else if in_string && c == string_char {
            // Check for escape
            if i > 0 && chars[i - 1] != '\\' {
                in_string = false;
            }
        } else if !in_string && c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            // Found comment start outside string
            return Some(&line[..i]);
        }
    }

    Some(line)
}

/// Strip block comments --[[ ... ]] and --[=[ ... ]=]
fn strip_block_comments(source: &str) -> (String, usize) {
    let mut result = source.to_string();
    let mut count = 0;

    // Handle --[[ ]] style
    let re = regex::Regex::new(r"--\[\[[\s\S]*?\]\]").unwrap();
    let matches: Vec<_> = re.find_iter(&result).collect();
    count += matches.len();
    result = re.replace_all(&result, "").to_string();

    // Handle --[=[ ]=] style (with varying = counts)
    let re2 = regex::Regex::new(r"--\[=+\[[\s\S]*?\]=+\]").unwrap();
    let matches2: Vec<_> = re2.find_iter(&result).collect();
    count += matches2.len();
    result = re2.replace_all(&result, "").to_string();

    (result, count)
}

/// Replace _0x style variable prefixes with a random prefix
fn replace_hex_prefixes(source: &str, new_prefix: &str) -> String {
    // Match patterns like _0x followed by hex characters (variable names)
    let re = regex::Regex::new(r"_0x([0-9a-fA-F]+)").unwrap();
    re.replace_all(source, format!("{}$1", new_prefix))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_encoding() {
        assert_eq!(to_hex_escaped("get"), "\\x67\\x65\\x74");
        assert_eq!(to_hex_escaped("A"), "\\x41");
    }

    #[test]
    fn test_encode_double_quoted_string() {
        let mut source = r#"game:GetService("InsertService")"#.to_string();
        let count = encode_string_occurrences(&mut source, "InsertService");
        assert_eq!(count, 1);
        assert!(source.contains("\\x49\\x6e\\x73\\x65\\x72\\x74")); // "Insert" hex
        assert!(!source.contains(r#""InsertService""#));
    }

    #[test]
    fn test_encode_single_quoted_string() {
        let mut source = "local x = 'getfenv'".to_string();
        let count = encode_string_occurrences(&mut source, "getfenv");
        assert_eq!(count, 1);
        assert!(source.contains("\\x67\\x65\\x74")); // "get" hex
    }

    #[test]
    fn test_strip_debug_prints() {
        let config = ObfuscatorConfig {
            debug: DebugConfig {
                strip_patterns: vec![r#"^\s*print\s*\(\s*"\[DEBUG"#.to_string()],
            },
            ..Default::default()
        };
        let obfuscator = Obfuscator::new(config);
        let source = r#"
local x = 5
print("[DEBUG] test")
local y = 10
"#;
        let result = obfuscator.obfuscate(source);
        assert!(!result.source.contains("[DEBUG]"));
        assert!(result.source.contains("local x = 5"));
        assert!(result.source.contains("local y = 10"));
        assert_eq!(result.debug_stripped, 1);
    }

    #[test]
    fn test_strip_line_comments() {
        let source = "local x = 5 -- this is a comment\nlocal y = 10";
        let (result, count) = strip_line_comments(source);
        assert!(!result.contains("this is a comment"));
        assert!(result.contains("local x = 5"));
        assert_eq!(count, 1);
    }

    #[test]
    fn test_preserve_string_with_dashes() {
        let source = r#"local s = "hello -- not a comment""#;
        let (result, count) = strip_line_comments(source);
        assert!(result.contains("hello -- not a comment"));
        assert_eq!(count, 0);
    }

    #[test]
    fn test_strip_block_comments() {
        let source = "local x = 5 --[[ block comment ]] local y = 10";
        let (result, count) = strip_block_comments(source);
        assert!(!result.contains("block comment"));
        assert!(result.contains("local x = 5"));
        assert!(result.contains("local y = 10"));
        assert_eq!(count, 1);
    }

    #[test]
    fn test_replace_hex_prefix() {
        let result = replace_hex_prefixes("local _0xABCD = 5", "_xy");
        assert!(result.contains("_xyABCD"));
        assert!(!result.contains("_0x"));
    }

    #[test]
    fn test_random_prefix_generation() {
        let prefix1 = generate_random_prefix();
        let prefix2 = generate_random_prefix();
        // Should be 3 chars: underscore + 2 letters
        assert_eq!(prefix1.len(), 3);
        assert_eq!(prefix2.len(), 3);
        assert!(prefix1.starts_with('_'));
        assert!(prefix2.starts_with('_'));
    }

    #[test]
    fn test_full_obfuscation() {
        let obfuscator = Obfuscator::with_defaults();
        let source = r#"
local service = game:GetService("InsertService")
local env = getfenv(0)
"#;
        let result = obfuscator.obfuscate(source);
        // InsertService should be hex-encoded
        assert!(!result.source.contains(r#""InsertService""#));
        assert!(result.source.contains("\\x")); // Has hex escapes
        // Original functionality keywords preserved (not in strings)
        assert!(result.source.contains("GetService"));
        assert!(result.source.contains("getfenv"));
    }

    #[test]
    fn test_no_encoding_of_bare_identifiers() {
        let obfuscator = Obfuscator::with_defaults();
        // getfenv as a function call, not a string - should NOT be encoded
        let source = "local env = getfenv(0)";
        let result = obfuscator.obfuscate(source);
        // The identifier getfenv should still be there as-is
        assert!(result.source.contains("getfenv(0)"));
        assert_eq!(result.strings_encoded, 0);
    }

    #[test]
    fn test_custom_config() {
        let config = ObfuscatorConfig {
            strings: StringConfig {
                encode: vec!["CustomAPI".to_string()],
            },
            debug: DebugConfig {
                strip_patterns: vec![],
            },
            minify: MinifyConfig {
                strip_comments: false,
                strip_block_comments: false,
            },
        };
        let obfuscator = Obfuscator::new(config);
        let source = r#"local x = "CustomAPI""#;
        let result = obfuscator.obfuscate(source);
        assert!(!result.source.contains(r#""CustomAPI""#));
        assert!(result.source.contains("\\x43")); // 'C' in hex
    }
}
