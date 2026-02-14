// Signal file handling
//
// Pure functional signal processing - monitors file modification times

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Get the version identifier for a signal file
///
/// Pure function (with file I/O side effect) - reads file mtime and converts to string
pub fn get_signal_version(comm_dir: &Path, signal_name: &str) -> Option<String> {
    let signal_path = comm_dir.join("signals").join(signal_name);
    get_file_mtime(&signal_path).map(format_system_time)
}

/// Get file modification time
///
/// Pure function (with file I/O side effect) - returns SystemTime
fn get_file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

/// Format SystemTime as version string
///
/// Pure function - converts SystemTime to string representation
fn format_system_time(time: SystemTime) -> String {
    format!("{:?}", time)
}

/// Check if signal file exists
///
/// Pure function (with file I/O side effect) - checks file existence
pub fn signal_exists(comm_dir: &Path, signal_name: &str) -> bool {
    let signal_path = comm_dir.join("signals").join(signal_name);
    signal_path.exists()
}

/// Create signal directory if it doesn't exist
///
/// Side effect function - creates directory
pub fn ensure_signal_dir(comm_dir: &Path) -> std::io::Result<PathBuf> {
    let signals_dir = comm_dir.join("signals");
    fs::create_dir_all(&signals_dir)?;
    Ok(signals_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_get_signal_version_returns_none_when_file_missing() {
        let temp_dir = TempDir::new().unwrap();
        let version = get_signal_version(temp_dir.path(), "nonexistent");

        assert_eq!(version, None);
    }

    #[test]
    fn test_get_signal_version_returns_some_when_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let signals_dir = temp_dir.path().join("signals");
        fs::create_dir(&signals_dir).unwrap();

        let signal_file = signals_dir.join("testSignal");
        fs::write(&signal_file, "").unwrap();

        let version = get_signal_version(temp_dir.path(), "testSignal");

        assert!(version.is_some());
        assert!(!version.unwrap().is_empty());
    }

    #[test]
    fn test_get_signal_version_changes_on_file_update() {
        let temp_dir = TempDir::new().unwrap();
        let signals_dir = temp_dir.path().join("signals");
        fs::create_dir(&signals_dir).unwrap();

        let signal_file = signals_dir.join("testSignal");
        fs::write(&signal_file, "v1").unwrap();

        let version1 = get_signal_version(temp_dir.path(), "testSignal");

        // Wait to ensure mtime changes
        thread::sleep(Duration::from_millis(10));

        fs::write(&signal_file, "v2").unwrap();

        let version2 = get_signal_version(temp_dir.path(), "testSignal");

        assert_ne!(version1, version2);
    }

    #[test]
    fn test_get_signal_version_stable_when_file_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let signals_dir = temp_dir.path().join("signals");
        fs::create_dir(&signals_dir).unwrap();

        let signal_file = signals_dir.join("testSignal");
        fs::write(&signal_file, "content").unwrap();

        let version1 = get_signal_version(temp_dir.path(), "testSignal");
        let version2 = get_signal_version(temp_dir.path(), "testSignal");

        assert_eq!(version1, version2);
    }

    #[test]
    fn test_signal_exists_returns_false_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        assert!(!signal_exists(temp_dir.path(), "missing"));
    }

    #[test]
    fn test_signal_exists_returns_true_when_present() {
        let temp_dir = TempDir::new().unwrap();
        let signals_dir = temp_dir.path().join("signals");
        fs::create_dir(&signals_dir).unwrap();

        let signal_file = signals_dir.join("testSignal");
        fs::write(&signal_file, "").unwrap();

        assert!(signal_exists(temp_dir.path(), "testSignal"));
    }

    #[test]
    fn test_ensure_signal_dir_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let signals_dir = ensure_signal_dir(temp_dir.path()).unwrap();

        assert!(signals_dir.exists());
        assert!(signals_dir.is_dir());
    }

    #[test]
    fn test_ensure_signal_dir_idempotent() {
        let temp_dir = TempDir::new().unwrap();

        let result1 = ensure_signal_dir(temp_dir.path());
        let result2 = ensure_signal_dir(temp_dir.path());

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert_eq!(result1.unwrap(), result2.unwrap());
    }

    #[test]
    fn test_format_system_time_produces_nonempty_string() {
        let time = SystemTime::now();
        let formatted = format_system_time(time);

        assert!(!formatted.is_empty());
    }

    #[test]
    fn test_format_system_time_different_times_different_strings() {
        let time1 = SystemTime::UNIX_EPOCH;
        let time2 = SystemTime::now();

        let formatted1 = format_system_time(time1);
        let formatted2 = format_system_time(time2);

        assert_ne!(formatted1, formatted2);
    }
}
