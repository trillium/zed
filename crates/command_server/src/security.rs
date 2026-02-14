// Security validation functions
//
// Pure functional security checks - all testable without filesystem

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityError {
    DirectoryIsSymlink,
    InvalidPermissions { actual: u32, expected: u32 },
    OwnershipMismatch { actual_uid: u32, expected_uid: u32 },
    ParentNotWritable,
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityError::DirectoryIsSymlink => write!(f, "Directory is a symlink"),
            SecurityError::InvalidPermissions { actual, expected } => {
                write!(f, "Invalid permissions: {:o} (expected {:o})", actual, expected)
            }
            SecurityError::OwnershipMismatch { actual_uid, expected_uid } => {
                write!(f, "Owner UID mismatch: {} (expected {})", actual_uid, expected_uid)
            }
            SecurityError::ParentNotWritable => write!(f, "Parent directory is not writable"),
        }
    }
}

impl std::error::Error for SecurityError {}

/// Validate directory permissions
///
/// Checks that the directory has mode 0700 (owner only)
#[cfg(unix)]
pub fn validate_directory_permissions(path: &Path) -> Result<(), SecurityError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|_| SecurityError::InvalidPermissions {
        actual: 0,
        expected: 0o700,
    })?;

    let mode = metadata.permissions().mode() & 0o777;
    let expected = 0o700;

    if mode == expected {
        Ok(())
    } else {
        Err(SecurityError::InvalidPermissions {
            actual: mode,
            expected,
        })
    }
}

/// Validate directory permissions (Windows stub)
///
/// Windows doesn't have Unix permissions, so this always succeeds
#[cfg(not(unix))]
pub fn validate_directory_permissions(_path: &Path) -> Result<(), SecurityError> {
    Ok(())
}

/// Check if path is a symlink
///
/// Pure function (with file I/O) - checks symlink status
pub fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Validate directory is not a symlink
///
/// Pure function - checks symlink status
pub fn validate_not_symlink(path: &Path) -> Result<(), SecurityError> {
    if is_symlink(path) {
        Err(SecurityError::DirectoryIsSymlink)
    } else {
        Ok(())
    }
}

/// Get current user ID
///
/// Pure function (platform-specific)
#[cfg(unix)]
pub fn get_current_uid() -> u32 {
    unsafe { libc::getuid() }
}

/// Get current user ID (Windows stub)
#[cfg(not(unix))]
pub fn get_current_uid() -> u32 {
    0 // Not applicable on Windows
}

/// Validate directory ownership
///
/// Checks that the directory is owned by the current user
#[cfg(unix)]
pub fn validate_directory_ownership(path: &Path) -> Result<(), SecurityError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path).map_err(|_| SecurityError::OwnershipMismatch {
        actual_uid: 0,
        expected_uid: get_current_uid(),
    })?;

    let actual_uid = metadata.uid();
    let expected_uid = get_current_uid();

    if actual_uid == expected_uid {
        Ok(())
    } else {
        Err(SecurityError::OwnershipMismatch {
            actual_uid,
            expected_uid,
        })
    }
}

/// Validate directory ownership (Windows stub)
#[cfg(not(unix))]
pub fn validate_directory_ownership(_path: &Path) -> Result<(), SecurityError> {
    Ok(())
}

/// Validate all security requirements for communication directory
///
/// Pure function - combines all security checks
pub fn validate_comm_directory(path: &Path) -> Result<(), SecurityError> {
    validate_not_symlink(path)?;
    validate_directory_permissions(path)?;
    validate_directory_ownership(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_symlink_returns_false_for_directory() {
        let temp_dir = TempDir::new().unwrap();
        assert!(!is_symlink(temp_dir.path()));
    }

    #[test]
    #[cfg(unix)]
    fn test_is_symlink_returns_true_for_symlink() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target");
        let link = temp_dir.path().join("link");

        fs::create_dir(&target).unwrap();
        unix_fs::symlink(&target, &link).unwrap();

        assert!(is_symlink(&link));
    }

    #[test]
    fn test_validate_not_symlink_accepts_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = validate_not_symlink(temp_dir.path());

        assert!(result.is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn test_validate_not_symlink_rejects_symlink() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target");
        let link = temp_dir.path().join("link");

        fs::create_dir(&target).unwrap();
        unix_fs::symlink(&target, &link).unwrap();

        let result = validate_not_symlink(&link);

        assert!(matches!(result, Err(SecurityError::DirectoryIsSymlink)));
    }

    #[test]
    #[cfg(unix)]
    fn test_validate_directory_permissions_accepts_0700() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir).unwrap();

        // Set permissions to 0700
        let mut perms = fs::metadata(&test_dir).unwrap().permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&test_dir, perms).unwrap();

        let result = validate_directory_permissions(&test_dir);

        assert!(result.is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn test_validate_directory_permissions_rejects_0755() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir).unwrap();

        // Set permissions to 0755 (too permissive)
        let mut perms = fs::metadata(&test_dir).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&test_dir, perms).unwrap();

        let result = validate_directory_permissions(&test_dir);

        assert!(matches!(result, Err(SecurityError::InvalidPermissions { .. })));
    }

    #[test]
    #[cfg(unix)]
    fn test_get_current_uid_returns_value() {
        let _uid = get_current_uid();
        // Just verify it returns without panicking
        // UID could be 0 (root) or any positive number
    }

    #[test]
    #[cfg(unix)]
    fn test_validate_directory_ownership_accepts_own_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = validate_directory_ownership(temp_dir.path());

        // Should succeed because we own the temp directory
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_comm_directory_accepts_valid_directory() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&test_dir).unwrap().permissions();
            perms.set_mode(0o700);
            fs::set_permissions(&test_dir, perms).unwrap();
        }

        let result = validate_comm_directory(&test_dir);

        assert!(result.is_ok());
    }

    #[test]
    fn test_security_error_display() {
        let err = SecurityError::DirectoryIsSymlink;
        assert_eq!(err.to_string(), "Directory is a symlink");

        let err = SecurityError::InvalidPermissions {
            actual: 0o755,
            expected: 0o700,
        };
        assert!(err.to_string().contains("755"));
        assert!(err.to_string().contains("700"));

        let err = SecurityError::OwnershipMismatch {
            actual_uid: 1000,
            expected_uid: 1001,
        };
        assert!(err.to_string().contains("1000"));
        assert!(err.to_string().contains("1001"));
    }
}
