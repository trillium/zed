// Command Server for Zed - File-based RPC for Talon voice control integration
//
// This module implements the command server infrastructure that allows
// external voice control systems (like Talon) to send commands to Zed
// via a secure file-based RPC mechanism.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub mod request;
pub mod response;
pub mod security;
pub mod signal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusedElementType {
    #[serde(rename = "textEditor")]
    TextEditor,
    #[serde(rename = "terminal")]
    Terminal,
    #[serde(rename = "other")]
    Other,
}

/// Command Server API implementation
///
/// Provides the required CommandServerApi interface that Cursorless
/// extensions need to communicate with voice control systems.
pub struct CommandServerApi {
    comm_dir: PathBuf,
    request_timeout: Duration,
}

impl CommandServerApi {
    /// Create a new CommandServerApi instance
    ///
    /// # Arguments
    /// * `comm_dir` - Communication directory path (e.g., /tmp/zed-command-server-{uid})
    /// * `request_timeout` - Maximum age for requests before rejection
    pub fn new(comm_dir: PathBuf, request_timeout: Duration) -> Self {
        Self {
            comm_dir,
            request_timeout,
        }
    }

    /// Get the focused element type
    ///
    /// This is a stub implementation that would need to be connected
    /// to actual Zed window focus tracking.
    pub fn get_focused_element_type(&self) -> Option<FocusedElementType> {
        // TODO: Connect to actual Zed focus tracking
        Some(FocusedElementType::TextEditor)
    }

    /// Get the pre-phrase signal version
    ///
    /// Returns the modification time of the signal file as a version string.
    /// When Talon touches this file before a new phrase, the version changes.
    pub fn get_pre_phrase_version(&self) -> Option<String> {
        signal::get_signal_version(&self.comm_dir, "prePhrase")
    }

    /// Get the communication directory path
    pub fn comm_dir(&self) -> &Path {
        &self.comm_dir
    }

    /// Get the configured request timeout
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_command_server_api_creation() {
        let temp_dir = TempDir::new().unwrap();
        let comm_dir = temp_dir.path().to_path_buf();
        let timeout = Duration::from_secs(3);

        let api = CommandServerApi::new(comm_dir.clone(), timeout);

        assert_eq!(api.comm_dir(), comm_dir.as_path());
        assert_eq!(api.request_timeout(), timeout);
    }

    #[test]
    fn test_get_focused_element_type_returns_text_editor() {
        let temp_dir = TempDir::new().unwrap();
        let api = CommandServerApi::new(temp_dir.path().to_path_buf(), Duration::from_secs(3));

        let focus_type = api.get_focused_element_type();

        assert_eq!(focus_type, Some(FocusedElementType::TextEditor));
    }

    #[test]
    fn test_focused_element_type_serialization() {
        let editor = FocusedElementType::TextEditor;
        let terminal = FocusedElementType::Terminal;
        let other = FocusedElementType::Other;

        let editor_json = serde_json::to_string(&editor).unwrap();
        let terminal_json = serde_json::to_string(&terminal).unwrap();
        let other_json = serde_json::to_string(&other).unwrap();

        assert_eq!(editor_json, "\"textEditor\"");
        assert_eq!(terminal_json, "\"terminal\"");
        assert_eq!(other_json, "\"other\"");
    }

    #[test]
    fn test_focused_element_type_deserialization() {
        let editor: FocusedElementType = serde_json::from_str("\"textEditor\"").unwrap();
        let terminal: FocusedElementType = serde_json::from_str("\"terminal\"").unwrap();
        let other: FocusedElementType = serde_json::from_str("\"other\"").unwrap();

        assert_eq!(editor, FocusedElementType::TextEditor);
        assert_eq!(terminal, FocusedElementType::Terminal);
        assert_eq!(other, FocusedElementType::Other);
    }

    #[test]
    fn test_get_pre_phrase_version_returns_none_when_no_signal() {
        let temp_dir = TempDir::new().unwrap();
        let api = CommandServerApi::new(temp_dir.path().to_path_buf(), Duration::from_secs(3));

        let version = api.get_pre_phrase_version();

        assert_eq!(version, None);
    }

    #[test]
    fn test_get_pre_phrase_version_returns_some_when_signal_exists() {
        let temp_dir = TempDir::new().unwrap();
        let signals_dir = temp_dir.path().join("signals");
        fs::create_dir(&signals_dir).unwrap();

        let signal_file = signals_dir.join("prePhrase");
        fs::write(&signal_file, "").unwrap();

        let api = CommandServerApi::new(temp_dir.path().to_path_buf(), Duration::from_secs(3));
        let version = api.get_pre_phrase_version();

        assert!(version.is_some());
        assert!(!version.unwrap().is_empty());
    }

    #[test]
    fn test_get_pre_phrase_version_changes_when_signal_updated() {
        let temp_dir = TempDir::new().unwrap();
        let signals_dir = temp_dir.path().join("signals");
        fs::create_dir(&signals_dir).unwrap();

        let signal_file = signals_dir.join("prePhrase");
        fs::write(&signal_file, "").unwrap();

        let api = CommandServerApi::new(temp_dir.path().to_path_buf(), Duration::from_secs(3));
        let version1 = api.get_pre_phrase_version();

        // Wait a bit and touch the file again
        std::thread::sleep(Duration::from_millis(10));
        fs::write(&signal_file, "updated").unwrap();

        let version2 = api.get_pre_phrase_version();

        assert_ne!(version1, version2);
    }
}
