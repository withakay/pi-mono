// Path utilities
use std::path::{Path, PathBuf};

/// Resolve a path that may be relative against a given base directory.
pub fn resolve_path(base: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

/// Return the default pi data directory: `~/.pi/rust-agent/`
pub fn pi_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pi")
        .join("rust-agent")
}

/// Return the sessions directory: `~/.pi/rust-agent/sessions/`
pub fn sessions_dir() -> PathBuf {
    pi_data_dir().join("sessions")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_absolute() {
        let base = Path::new("/tmp");
        let result = resolve_path(base, "/etc/hosts");
        assert_eq!(result, PathBuf::from("/etc/hosts"));
    }

    #[test]
    fn test_resolve_relative() {
        let base = Path::new("/tmp/work");
        let result = resolve_path(base, "file.txt");
        assert_eq!(result, PathBuf::from("/tmp/work/file.txt"));
    }
}

