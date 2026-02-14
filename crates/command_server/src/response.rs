// Response serialization
//
// Pure functional response handling - no side effects, fully testable

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub uuid: String,
    pub return_value: Option<serde_json::Value>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
}

impl Response {
    /// Create a success response
    ///
    /// Pure function - constructs response with return value
    pub fn success(uuid: String, return_value: Option<serde_json::Value>) -> Self {
        Self {
            uuid,
            return_value,
            error: None,
            warnings: Vec::new(),
        }
    }

    /// Create an error response
    ///
    /// Pure function - constructs response with error message
    pub fn error(uuid: String, error: String) -> Self {
        Self {
            uuid,
            return_value: None,
            error: Some(error),
            warnings: Vec::new(),
        }
    }

    /// Add a warning to the response
    ///
    /// Pure function - creates new response with warning added
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Serialize to JSON string
    ///
    /// Pure function - converts response to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize to JSON with single-line format and trailing newline
    ///
    /// This matches the VSCode command-server format requirement
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(format!("{}\n", json))
    }

    /// Parse from JSON string
    ///
    /// Pure function - parses JSON into response
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_response() {
        let response = Response::success(
            "test-uuid".to_string(),
            Some(serde_json::json!({"result": "ok"})),
        );

        assert_eq!(response.uuid, "test-uuid");
        assert_eq!(response.return_value, Some(serde_json::json!({"result": "ok"})));
        assert_eq!(response.error, None);
        assert!(response.warnings.is_empty());
    }

    #[test]
    fn test_success_response_no_return_value() {
        let response = Response::success("test-uuid".to_string(), None);

        assert_eq!(response.uuid, "test-uuid");
        assert_eq!(response.return_value, None);
        assert_eq!(response.error, None);
    }

    #[test]
    fn test_error_response() {
        let response = Response::error("test-uuid".to_string(), "Something went wrong".to_string());

        assert_eq!(response.uuid, "test-uuid");
        assert_eq!(response.return_value, None);
        assert_eq!(response.error, Some("Something went wrong".to_string()));
        assert!(response.warnings.is_empty());
    }

    #[test]
    fn test_response_with_warning() {
        let response = Response::success("test-uuid".to_string(), None)
            .with_warning("First warning".to_string())
            .with_warning("Second warning".to_string());

        assert_eq!(response.warnings.len(), 2);
        assert_eq!(response.warnings[0], "First warning");
        assert_eq!(response.warnings[1], "Second warning");
    }

    #[test]
    fn test_to_json() {
        let response = Response::success("test-uuid".to_string(), Some(serde_json::json!(42)));

        let json = response.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["uuid"], "test-uuid");
        assert_eq!(parsed["returnValue"], 42);
        assert_eq!(parsed["error"], serde_json::Value::Null);
    }

    #[test]
    fn test_to_json_line_has_trailing_newline() {
        let response = Response::success("test-uuid".to_string(), None);

        let json_line = response.to_json_line().unwrap();

        assert!(json_line.ends_with('\n'));
        assert_eq!(json_line.matches('\n').count(), 1); // Exactly one newline
    }

    #[test]
    fn test_to_json_line_single_line() {
        let response = Response::success("test-uuid".to_string(), Some(serde_json::json!({"nested": {"data": [1, 2, 3]}})));

        let json_line = response.to_json_line().unwrap();

        // Should not contain internal newlines (compact format)
        assert_eq!(json_line.matches('\n').count(), 1);
    }

    #[test]
    fn test_from_json() {
        let json = r#"{"uuid":"test-uuid","returnValue":{"status":"ok"},"error":null,"warnings":["warning1"]}"#;

        let response = Response::from_json(json).unwrap();

        assert_eq!(response.uuid, "test-uuid");
        assert_eq!(response.return_value, Some(serde_json::json!({"status": "ok"})));
        assert_eq!(response.error, None);
        assert_eq!(response.warnings, vec!["warning1"]);
    }

    #[test]
    fn test_response_roundtrip() {
        let original = Response::success("test-uuid".to_string(), Some(serde_json::json!({"key": "value"})))
            .with_warning("test warning".to_string());

        let json = original.to_json().unwrap();
        let parsed = Response::from_json(&json).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_response_serialization_format() {
        let response = Response::success("550e8400-e29b-41d4-a716-446655440000".to_string(), None);

        let json = response.to_json().unwrap();

        // Verify camelCase
        assert!(json.contains("\"returnValue\""));
        assert!(!json.contains("\"return_value\""));
    }

    #[test]
    fn test_error_response_serialization() {
        let response = Response::error("test-uuid".to_string(), "Error message".to_string());

        let json = response.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["error"], "Error message");
        assert_eq!(parsed["returnValue"], serde_json::Value::Null);
    }
}
