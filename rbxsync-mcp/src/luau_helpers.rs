//! Shared helper functions for generating Luau code in MCP tools.
//!
//! These utilities handle string escaping, identifier validation, path navigation,
//! and JSON-to-Luau conversion. They are used by the various MCP tool implementations
//! that generate Luau code to execute in Roblox Studio.

/// Escape a string for safe use inside a Luau double-quoted string literal.
///
/// Escapes backslashes, double quotes, newlines, tabs, and carriage returns.
pub fn escape_luau_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

/// Validate that a string is a safe Luau identifier.
///
/// A valid Luau identifier matches `^[A-Za-z_][A-Za-z0-9_]*$`.
/// This prevents Luau injection when property names, class names, or attribute names
/// are interpolated into generated code.
///
/// Returns `Ok(())` if valid, or `Err` with a descriptive message if not.
pub fn validate_luau_identifier(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("Identifier cannot be empty".to_string());
    }

    let mut chars = s.chars();

    // First character must be a letter or underscore
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        Some(c) => {
            return Err(format!(
                "Invalid identifier '{}': must start with a letter or underscore, got '{}'",
                s, c
            ));
        }
        None => unreachable!(), // Already checked is_empty
    }

    // Remaining characters must be alphanumeric or underscore
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(format!(
                "Invalid identifier '{}': contains invalid character '{}'",
                s, c
            ));
        }
    }

    Ok(())
}

/// Generate Luau code that navigates from `game` to an instance at the given path.
///
/// Splits the path by `/`, uses `GetService` for the first component,
/// then walks through `FindFirstChild` calls for each subsequent component.
/// Returns a Luau snippet defining a local `navigate` function.
pub fn luau_navigate_snippet(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.is_empty() {
        return "local target = game".to_string();
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "local target = game:GetService(\"{}\")",
        escape_luau_string(parts[0])
    ));

    for part in &parts[1..] {
        lines.push(format!(
            "target = target and target:FindFirstChild(\"{}\")",
            escape_luau_string(part)
        ));
    }

    lines.join("\n")
}

/// Convert a JSON value to a Luau literal expression.
///
/// Handles:
/// - `null` -> `nil`
/// - `bool` -> `true`/`false`
/// - `number` -> number literal
/// - `string` -> escaped double-quoted string literal (always quoted, never raw Luau)
/// - `array` -> `{item1, item2, ...}`
/// - `object` -> `{key1 = val1, key2 = val2, ...}` (keys must be valid identifiers)
///
/// **Security note:** Strings are always quoted. If callers need Roblox constructors
/// (e.g., `Vector3.new(...)`, `Enum.Material.Plastic`), they should construct those
/// via a separate parameter or explicit API, not by passing constructor expressions as
/// string values.
pub fn json_value_to_luau(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "nil".to_string(),
        serde_json::Value::Bool(b) => format!("{}", b),
        serde_json::Value::Number(n) => format!("{}", n),
        serde_json::Value::String(s) => {
            format!("\"{}\"", escape_luau_string(s))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_value_to_luau).collect();
            format!("{{{}}}", items.join(", "))
        }
        serde_json::Value::Object(map) => {
            let mut entries = Vec::new();
            for (key, val) in map {
                if validate_luau_identifier(key).is_ok() {
                    entries.push(format!("{} = {}", key, json_value_to_luau(val)));
                } else {
                    // Use bracket notation for non-identifier keys
                    entries.push(format!(
                        "[\"{}\"] = {}",
                        escape_luau_string(key),
                        json_value_to_luau(val)
                    ));
                }
            }
            format!("{{{}}}", entries.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // escape_luau_string tests
    // ========================================================================

    #[test]
    fn test_escape_luau_string_plain() {
        assert_eq!(escape_luau_string("hello"), "hello");
    }

    #[test]
    fn test_escape_luau_string_backslash() {
        assert_eq!(escape_luau_string(r"a\b"), r"a\\b");
    }

    #[test]
    fn test_escape_luau_string_double_quote() {
        assert_eq!(escape_luau_string(r#"say "hi""#), r#"say \"hi\""#);
    }

    #[test]
    fn test_escape_luau_string_newline_tab_cr() {
        assert_eq!(escape_luau_string("a\nb\tc\r"), "a\\nb\\tc\\r");
    }

    #[test]
    fn test_escape_luau_string_combined() {
        assert_eq!(
            escape_luau_string("line1\nline2\t\"end\"\\done"),
            "line1\\nline2\\t\\\"end\\\"\\\\done"
        );
    }

    #[test]
    fn test_escape_luau_string_empty() {
        assert_eq!(escape_luau_string(""), "");
    }

    // ========================================================================
    // validate_luau_identifier tests
    // ========================================================================

    #[test]
    fn test_valid_identifiers() {
        assert!(validate_luau_identifier("x").is_ok());
        assert!(validate_luau_identifier("_foo").is_ok());
        assert!(validate_luau_identifier("FooBar123").is_ok());
        assert!(validate_luau_identifier("__double").is_ok());
        assert!(validate_luau_identifier("a").is_ok());
        assert!(validate_luau_identifier("Z").is_ok());
        assert!(validate_luau_identifier("_").is_ok());
        assert!(validate_luau_identifier("snake_case_99").is_ok());
    }

    #[test]
    fn test_invalid_identifiers() {
        assert!(validate_luau_identifier("").is_err());
        assert!(validate_luau_identifier("1abc").is_err());
        assert!(validate_luau_identifier("foo bar").is_err());
        assert!(validate_luau_identifier("foo-bar").is_err());
        assert!(validate_luau_identifier("foo.bar").is_err());
        assert!(validate_luau_identifier("foo\"bar").is_err());
        assert!(validate_luau_identifier("123").is_err());
        assert!(validate_luau_identifier("-leading").is_err());
    }

    #[test]
    fn test_identifier_rejects_injection() {
        // These would be dangerous if interpolated as raw identifiers in Luau code
        assert!(validate_luau_identifier("Name; Destroy()").is_err());
        assert!(validate_luau_identifier("end\nprint('pwned')").is_err());
    }

    // ========================================================================
    // luau_navigate_snippet tests
    // ========================================================================

    #[test]
    fn test_navigate_single_service() {
        let snippet = luau_navigate_snippet("Workspace");
        assert_eq!(snippet, "local target = game:GetService(\"Workspace\")");
    }

    #[test]
    fn test_navigate_nested_path() {
        let snippet = luau_navigate_snippet("ServerScriptService/MyFolder/Script");
        assert!(snippet.contains("game:GetService(\"ServerScriptService\")"));
        assert!(snippet.contains("FindFirstChild(\"MyFolder\")"));
        assert!(snippet.contains("FindFirstChild(\"Script\")"));
    }

    #[test]
    fn test_navigate_escapes_special_chars() {
        let snippet = luau_navigate_snippet("Workspace/My \"Part\"");
        assert!(snippet.contains(r#"FindFirstChild("My \"Part\"")"#));
    }

    // ========================================================================
    // json_value_to_luau tests
    // ========================================================================

    #[test]
    fn test_null_to_luau() {
        assert_eq!(json_value_to_luau(&serde_json::Value::Null), "nil");
    }

    #[test]
    fn test_bool_to_luau() {
        assert_eq!(
            json_value_to_luau(&serde_json::Value::Bool(true)),
            "true"
        );
        assert_eq!(
            json_value_to_luau(&serde_json::Value::Bool(false)),
            "false"
        );
    }

    #[test]
    fn test_number_to_luau() {
        assert_eq!(
            json_value_to_luau(&serde_json::json!(42)),
            "42"
        );
        assert_eq!(
            json_value_to_luau(&serde_json::json!(3.14)),
            "3.14"
        );
    }

    #[test]
    fn test_string_to_luau() {
        assert_eq!(
            json_value_to_luau(&serde_json::json!("hello")),
            "\"hello\""
        );
    }

    #[test]
    fn test_string_always_quoted_no_passthrough() {
        // Strings that look like Roblox constructors should NOT be passed through as raw Luau.
        // This was a security issue in the original per-PR implementations.
        assert_eq!(
            json_value_to_luau(&serde_json::json!("Enum.Material.Plastic")),
            "\"Enum.Material.Plastic\""
        );
        assert_eq!(
            json_value_to_luau(&serde_json::json!("Vector3.new(0,0,0)")),
            "\"Vector3.new(0,0,0)\""
        );
        assert_eq!(
            json_value_to_luau(&serde_json::json!("Color3.new(1,0,0)")),
            "\"Color3.new(1,0,0)\""
        );
    }

    #[test]
    fn test_string_escapes_special_chars() {
        assert_eq!(
            json_value_to_luau(&serde_json::json!("say \"hi\"")),
            r#""say \"hi\"""#
        );
        assert_eq!(
            json_value_to_luau(&serde_json::json!("line1\nline2")),
            r#""line1\nline2""#
        );
    }

    #[test]
    fn test_array_to_luau() {
        assert_eq!(
            json_value_to_luau(&serde_json::json!([1, "two", true])),
            "{1, \"two\", true}"
        );
    }

    #[test]
    fn test_empty_array_to_luau() {
        assert_eq!(json_value_to_luau(&serde_json::json!([])), "{}");
    }

    #[test]
    fn test_object_to_luau() {
        let val = serde_json::json!({"Anchored": true});
        let result = json_value_to_luau(&val);
        assert_eq!(result, "{Anchored = true}");
    }

    #[test]
    fn test_object_with_invalid_key_uses_bracket_notation() {
        let val = serde_json::json!({"some-key": 42});
        let result = json_value_to_luau(&val);
        assert_eq!(result, r#"{["some-key"] = 42}"#);
    }

    #[test]
    fn test_nested_structure_to_luau() {
        let val = serde_json::json!({"items": [1, 2], "name": "test"});
        let result = json_value_to_luau(&val);
        // Object iteration order may vary, so check both parts
        assert!(result.contains("items = {1, 2}"));
        assert!(result.contains("name = \"test\""));
    }
}
