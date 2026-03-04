// Session persistence - JSONL format for TypeScript compatibility
use super::messages::{SessionEntry};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Session manager handles loading and saving sessions in JSONL format
pub struct SessionManager {
    session_dir: PathBuf,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(session_dir: PathBuf) -> Self {
        Self { session_dir }
    }

    /// Get the path for a session file
    pub fn session_path(&self, session_id: &str) -> PathBuf {
        self.session_dir.join(format!("{}.jsonl", session_id))
    }

    /// Load all entries from a session file
    pub async fn load_session(&self, session_id: &str) -> Result<Vec<SessionEntry>> {
        let path = self.session_path(session_id);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&path)
            .await
            .with_context(|| format!("Failed to open session file: {}", path.display()))?;

        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut entries = Vec::new();

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            let entry: SessionEntry = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse session entry: {}", line))?;

            entries.push(entry);
        }

        Ok(entries)
    }

    /// Append a new entry to a session file
    pub async fn append_entry(&self, session_id: &str, entry: &SessionEntry) -> Result<()> {
        // Ensure session directory exists
        tokio::fs::create_dir_all(&self.session_dir)
            .await
            .context("Failed to create session directory")?;

        let path = self.session_path(session_id);

        // Open file in append mode
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .with_context(|| format!("Failed to open session file for append: {}", path.display()))?;

        // Serialize entry to JSON and write with newline
        let json = serde_json::to_string(entry)
            .context("Failed to serialize session entry")?;

        file.write_all(json.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;

        Ok(())
    }

    /// Create a new session with the given ID
    pub async fn create_session(&self, session_id: &str) -> Result<()> {
        // Ensure session directory exists
        tokio::fs::create_dir_all(&self.session_dir)
            .await
            .context("Failed to create session directory")?;

        let path = self.session_path(session_id);

        // Create empty file
        File::create(&path)
            .await
            .with_context(|| format!("Failed to create session file: {}", path.display()))?;

        Ok(())
    }

    /// List all session IDs
    pub async fn list_sessions(&self) -> Result<Vec<String>> {
        if !self.session_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = tokio::fs::read_dir(&self.session_dir)
            .await
            .context("Failed to read session directory")?;

        let mut sessions = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    sessions.push(stem.to_string());
                }
            }
        }

        Ok(sessions)
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let path = self.session_path(session_id);

        if path.exists() {
            tokio::fs::remove_file(&path)
                .await
                .with_context(|| format!("Failed to delete session: {}", path.display()))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::messages::{Message, MessageRole, MessageContent};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_and_load_session() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().to_path_buf());

        let session_id = "test-session";
        manager.create_session(session_id).await.unwrap();

        // Add some entries
        let msg1 = Message {
            id: "1".to_string(),
            parent_id: None,
            role: MessageRole::User,
            content: MessageContent::Text("Hello".to_string()),
            timestamp: Some(1234567890),
            model: None,
            stop_reason: None,
            metadata: None,
        };

        let entry1 = SessionEntry::Message(msg1);
        manager.append_entry(session_id, &entry1).await.unwrap();

        // Load and verify
        let entries = manager.load_session(session_id).await.unwrap();
        assert_eq!(entries.len(), 1);

        match &entries[0] {
            SessionEntry::Message(msg) => {
                assert_eq!(msg.id, "1");
                assert_eq!(msg.role, MessageRole::User);
            }
            _ => panic!("Expected Message entry"),
        }
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().to_path_buf());

        manager.create_session("session1").await.unwrap();
        manager.create_session("session2").await.unwrap();
        manager.create_session("session3").await.unwrap();

        let sessions = manager.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 3);
        assert!(sessions.contains(&"session1".to_string()));
        assert!(sessions.contains(&"session2".to_string()));
        assert!(sessions.contains(&"session3".to_string()));
    }

    #[tokio::test]
    async fn test_delete_session() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().to_path_buf());

        let session_id = "test-session";
        manager.create_session(session_id).await.unwrap();

        // Verify it exists
        let sessions = manager.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);

        // Delete it
        manager.delete_session(session_id).await.unwrap();

        // Verify it's gone
        let sessions = manager.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_append_multiple_entries() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().to_path_buf());

        let session_id = "multi-entry-session";
        manager.create_session(session_id).await.unwrap();

        // Add multiple entries
        for i in 1..=5 {
            let msg = Message {
                id: i.to_string(),
                parent_id: if i > 1 { Some((i - 1).to_string()) } else { None },
                role: if i % 2 == 1 { MessageRole::User } else { MessageRole::Assistant },
                content: MessageContent::Text(format!("Message {}", i)),
                timestamp: Some(1234567890 + i as i64),
                model: None,
                stop_reason: None,
                metadata: None,
            };

            let entry = SessionEntry::Message(msg);
            manager.append_entry(session_id, &entry).await.unwrap();
        }

        // Load and verify all entries
        let entries = manager.load_session(session_id).await.unwrap();
        assert_eq!(entries.len(), 5);

        // Verify order and content
        for (i, entry) in entries.iter().enumerate() {
            match entry {
                SessionEntry::Message(msg) => {
                    assert_eq!(msg.id, (i + 1).to_string());
                }
                _ => panic!("Expected Message entry"),
            }
        }
    }

    #[tokio::test]
    async fn test_load_nonexistent_session() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().to_path_buf());

        // Loading a session that doesn't exist should return empty vec
        let entries = manager.load_session("nonexistent").await.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_list_sessions_nonexistent_dir() {
        let manager = SessionManager::new(std::path::PathBuf::from("/tmp/nonexistent_session_dir_xyz"));

        // Listing sessions in a nonexistent directory should return empty vec
        let sessions = manager.list_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_session() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().to_path_buf());

        // Deleting a nonexistent session should not error
        manager.delete_session("nonexistent").await.unwrap();
    }
}
