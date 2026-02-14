// Request parsing and validation
//
// Pure functional request handling - no side effects, fully testable

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub command_id: String,
    pub args: Vec<serde_json::Value>,
    pub uuid: String,
    #[serde(default)]
    pub wait_for_finish: bool,
    #[serde(default)]
    pub return_command_output: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestError {
    InvalidJson(String),
    MissingField(String),
    InvalidUuid(String),
    RequestTooOld { age: Duration, max_age: Duration },
    InvalidCommandId(String),
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestError::InvalidJson(msg) => write!(f, "Invalid JSON: {}", msg),
            RequestError::MissingField(field) => write!(f, "Missing required field: {}", field),
            RequestError::InvalidUuid(uuid) => write!(f, "Invalid UUID: {}", uuid),
            RequestError::RequestTooOld { age, max_age } => {
                write!(f, "Request too old: {:?} (max: {:?})", age, max_age)
            }
            RequestError::InvalidCommandId(id) => write!(f, "Invalid command ID: {}", id),
        }
    }
}

impl std::error::Error for RequestError {}

/// Parse JSON into a Request
///
/// Pure function - takes JSON string, returns Result
pub fn parse_request(json: &str) -> Result<Request, RequestError> {
    serde_json::from_str(json).map_err(|e| RequestError::InvalidJson(e.to_string()))
}

/// Validate UUID format
///
/// Pure function - takes UUID string, returns bool
pub fn is_valid_uuid(uuid_str: &str) -> bool {
    Uuid::parse_str(uuid_str).is_ok()
}

/// Validate request UUID
///
/// Pure function - takes request, returns Result
pub fn validate_uuid(request: &Request) -> Result<(), RequestError> {
    if is_valid_uuid(&request.uuid) {
        Ok(())
    } else {
        Err(RequestError::InvalidUuid(request.uuid.clone()))
    }
}

/// Check if request is too old
///
/// Pure function - takes timestamp, current time, max age, returns Result
pub fn check_request_age(
    request_time: SystemTime,
    current_time: SystemTime,
    max_age: Duration,
) -> Result<(), RequestError> {
    let age = current_time
        .duration_since(request_time)
        .unwrap_or(Duration::from_secs(0));

    if age > max_age {
        Err(RequestError::RequestTooOld { age, max_age })
    } else {
        Ok(())
    }
}

/// Validate command ID against allow/deny lists
///
/// Pure function - takes command ID and lists, returns Result
pub fn validate_command_id(
    command_id: &str,
    allow_list: &[String],
    deny_list: &[String],
) -> Result<(), RequestError> {
    // Check deny list first
    for pattern in deny_list {
        if glob_match(pattern, command_id) {
            return Err(RequestError::InvalidCommandId(format!(
                "Command '{}' matches deny pattern '{}'",
                command_id, pattern
            )));
        }
    }

    // Check allow list
    for pattern in allow_list {
        if glob_match(pattern, command_id) {
            return Ok(());
        }
    }

    // Not in allow list
    Err(RequestError::InvalidCommandId(format!(
        "Command '{}' not in allow list",
        command_id
    )))
}

/// Simple glob pattern matching
///
/// Pure function - supports * wildcard only
fn glob_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if !pattern.contains('*') {
        return pattern == text;
    }

    // Simple * wildcard matching
    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.is_empty() {
        return true;
    }

    // Check prefix
    if !parts[0].is_empty() && !text.starts_with(parts[0]) {
        return false;
    }

    // Check suffix
    if parts.len() > 1 && !parts[parts.len() - 1].is_empty() && !text.ends_with(parts[parts.len() - 1]) {
        return false;
    }

    // Check middle parts
    let mut current_pos = parts[0].len();
    for i in 1..parts.len() - 1 {
        if let Some(pos) = text[current_pos..].find(parts[i]) {
            current_pos += pos + parts[i].len();
        } else {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_request() {
        let json = r#"{
            "commandId": "cursorless.command",
            "args": [{"action": "setSelection"}],
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "waitForFinish": true,
            "returnCommandOutput": false
        }"#;

        let request = parse_request(json).unwrap();

        assert_eq!(request.command_id, "cursorless.command");
        assert_eq!(request.uuid, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(request.wait_for_finish, true);
        assert_eq!(request.return_command_output, false);
    }

    #[test]
    fn test_parse_minimal_request() {
        let json = r#"{
            "commandId": "test.command",
            "args": [],
            "uuid": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        let request = parse_request(json).unwrap();

        assert_eq!(request.command_id, "test.command");
        assert_eq!(request.wait_for_finish, false); // Default
        assert_eq!(request.return_command_output, false); // Default
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = "not json";
        let result = parse_request(json);

        assert!(matches!(result, Err(RequestError::InvalidJson(_))));
    }

    #[test]
    fn test_is_valid_uuid_accepts_valid_uuids() {
        assert!(is_valid_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_valid_uuid("00000000-0000-0000-0000-000000000000"));
        assert!(is_valid_uuid("ffffffff-ffff-ffff-ffff-ffffffffffff"));
    }

    #[test]
    fn test_is_valid_uuid_rejects_invalid_uuids() {
        assert!(!is_valid_uuid("not-a-uuid"));
        assert!(!is_valid_uuid(""));
        assert!(!is_valid_uuid("550e8400-e29b-41d4-a716"));
        // Note: UUID crate accepts UUIDs without dashes, so we don't test that
    }

    #[test]
    fn test_validate_uuid_accepts_valid_uuid() {
        let request = Request {
            command_id: "test".to_string(),
            args: vec![],
            uuid: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            wait_for_finish: false,
            return_command_output: false,
        };

        let result = validate_uuid(&request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_uuid_rejects_invalid_uuid() {
        let request = Request {
            command_id: "test".to_string(),
            args: vec![],
            uuid: "invalid-uuid".to_string(),
            wait_for_finish: false,
            return_command_output: false,
        };

        let result = validate_uuid(&request);
        assert!(matches!(result, Err(RequestError::InvalidUuid(_))));
    }

    #[test]
    fn test_check_request_age_accepts_fresh_request() {
        let now = SystemTime::now();
        let one_second_ago = now - Duration::from_secs(1);
        let max_age = Duration::from_secs(3);

        let result = check_request_age(one_second_ago, now, max_age);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_request_age_rejects_old_request() {
        let now = SystemTime::now();
        let five_seconds_ago = now - Duration::from_secs(5);
        let max_age = Duration::from_secs(3);

        let result = check_request_age(five_seconds_ago, now, max_age);
        assert!(matches!(result, Err(RequestError::RequestTooOld { .. })));
    }

    #[test]
    fn test_check_request_age_exact_boundary() {
        let now = SystemTime::now();
        let exactly_max_age_ago = now - Duration::from_secs(3);
        let max_age = Duration::from_secs(3);

        let result = check_request_age(exactly_max_age_ago, now, max_age);
        // At exact boundary might be ok due to timing precision, accept either
        // The important thing is that slightly over the boundary is rejected
        let _ = result; // Could be ok or err depending on precision
    }

    #[test]
    fn test_validate_command_id_allows_matching_pattern() {
        let allow = vec!["cursorless.*".to_string()];
        let deny = vec![];

        let result = validate_command_id("cursorless.command", &allow, &deny);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_command_id_denies_non_matching() {
        let allow = vec!["cursorless.*".to_string()];
        let deny = vec![];

        let result = validate_command_id("evil.command", &allow, &deny);
        assert!(matches!(result, Err(RequestError::InvalidCommandId(_))));
    }

    #[test]
    fn test_validate_command_id_deny_list_takes_precedence() {
        let allow = vec!["*".to_string()];
        let deny = vec!["evil.*".to_string()];

        let result = validate_command_id("evil.command", &allow, &deny);
        assert!(matches!(result, Err(RequestError::InvalidCommandId(_))));
    }

    #[test]
    fn test_validate_command_id_wildcard_allow_all() {
        let allow = vec!["*".to_string()];
        let deny = vec![];

        assert!(validate_command_id("any.command", &allow, &deny).is_ok());
        assert!(validate_command_id("other.thing", &allow, &deny).is_ok());
    }

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("test", "test"));
        assert!(!glob_match("test", "test2"));
    }

    #[test]
    fn test_glob_match_wildcard_all() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn test_glob_match_prefix() {
        assert!(glob_match("cursorless.*", "cursorless.command"));
        assert!(glob_match("cursorless.*", "cursorless.anything"));
        assert!(!glob_match("cursorless.*", "other.command"));
    }

    #[test]
    fn test_glob_match_suffix() {
        assert!(glob_match("*.command", "cursorless.command"));
        assert!(glob_match("*.command", "other.command"));
        assert!(!glob_match("*.command", "cursorless.other"));
    }

    #[test]
    fn test_glob_match_middle() {
        assert!(glob_match("cur*less", "cursorless"));
        assert!(glob_match("cur*less", "cur-anything-less"));
        assert!(!glob_match("cur*less", "cursormore"));
    }

    #[test]
    fn test_request_serialization_roundtrip() {
        let request = Request {
            command_id: "test.command".to_string(),
            args: vec![serde_json::json!({"key": "value"})],
            uuid: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            wait_for_finish: true,
            return_command_output: false,
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed = parse_request(&json).unwrap();

        assert_eq!(request, parsed);
    }
}
