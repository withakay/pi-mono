// Settings management
// Based on TypeScript implementation but using TOML instead of JSON

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Context, Result};

/// Compaction settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub reserve_tokens: usize,
    pub keep_recent_tokens: usize,
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            reserve_tokens: 16384,
            keep_recent_tokens: 20000,
        }
    }
}

/// Branch summary settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BranchSummarySettings {
    pub reserve_tokens: usize,
}

impl Default for BranchSummarySettings {
    fn default() -> Self {
        Self {
            reserve_tokens: 16384,
        }
    }
}

/// Retry settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetrySettings {
    pub enabled: bool,
    pub max_retries: usize,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetrySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            base_delay_ms: 2000,
            max_delay_ms: 60000,
        }
    }
}

/// Terminal settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalSettings {
    pub show_images: bool,
    pub clear_on_shrink: bool,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            show_images: true,
            clear_on_shrink: false,
        }
    }
}

/// Thinking level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl Default for ThinkingLevel {
    fn default() -> Self {
        Self::Medium
    }
}

/// Main settings structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // Model configuration
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub default_thinking_level: ThinkingLevel,

    // Transport and mode
    pub transport: String,
    pub steering_mode: String,
    pub follow_up_mode: String,

    // Theme
    pub theme: String,

    // Feature settings
    pub compaction: CompactionSettings,
    pub branch_summary: BranchSummarySettings,
    pub retry: RetrySettings,
    pub terminal: TerminalSettings,

    // UI settings
    pub hide_thinking_block: bool,
    pub quiet_startup: bool,
    pub collapse_changelog: bool,

    // Shell customization
    pub shell_path: Option<String>,
    pub shell_command_prefix: Option<String>,

    // Editor settings
    pub editor_padding_x: usize,
    pub autocomplete_max_visible: usize,
    pub show_hardware_cursor: bool,

    // Extensions and packages
    pub extensions: Vec<String>,
    pub skills: Vec<String>,
    pub prompts: Vec<String>,
    pub themes: Vec<String>,
    pub enable_skill_commands: bool,

    // Model cycling
    pub enabled_models: Vec<String>,
    pub double_escape_action: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_provider: None,
            default_model: None,
            default_thinking_level: ThinkingLevel::Medium,
            transport: "sse".to_string(),
            steering_mode: "one-at-a-time".to_string(),
            follow_up_mode: "one-at-a-time".to_string(),
            theme: "dark".to_string(),
            compaction: CompactionSettings::default(),
            branch_summary: BranchSummarySettings::default(),
            retry: RetrySettings::default(),
            terminal: TerminalSettings::default(),
            hide_thinking_block: false,
            quiet_startup: false,
            collapse_changelog: false,
            shell_path: None,
            shell_command_prefix: None,
            editor_padding_x: 0,
            autocomplete_max_visible: 5,
            show_hardware_cursor: false,
            extensions: Vec::new(),
            skills: Vec::new(),
            prompts: Vec::new(),
            themes: Vec::new(),
            enable_skill_commands: true,
            enabled_models: Vec::new(),
            double_escape_action: "tree".to_string(),
        }
    }
}

impl Settings {
    /// Load settings from a file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read settings from {:?}", path))?;

        let settings: Self = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse settings from {:?}", path))?;

        Ok(settings)
    }

    /// Save settings to a file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }

        let contents = toml::to_string_pretty(self)
            .context("Failed to serialize settings")?;

        fs::write(path, contents)
            .with_context(|| format!("Failed to write settings to {:?}", path))?;

        Ok(())
    }

    /// Merge settings from another Settings instance
    /// Other settings take precedence
    pub fn merge(&mut self, other: &Settings) {
        // Only override if other has non-default values
        if other.default_provider.is_some() {
            self.default_provider = other.default_provider.clone();
        }
        if other.default_model.is_some() {
            self.default_model = other.default_model.clone();
        }

        // Simple field overrides
        self.default_thinking_level = other.default_thinking_level;
        self.transport = other.transport.clone();
        self.steering_mode = other.steering_mode.clone();
        self.follow_up_mode = other.follow_up_mode.clone();
        self.theme = other.theme.clone();
        self.hide_thinking_block = other.hide_thinking_block;
        self.quiet_startup = other.quiet_startup;
        self.collapse_changelog = other.collapse_changelog;

        // Merge nested settings
        self.compaction = other.compaction.clone();
        self.branch_summary = other.branch_summary.clone();
        self.retry = other.retry.clone();
        self.terminal = other.terminal.clone();

        // Optional fields
        if other.shell_path.is_some() {
            self.shell_path = other.shell_path.clone();
        }
        if other.shell_command_prefix.is_some() {
            self.shell_command_prefix = other.shell_command_prefix.clone();
        }

        // Arrays - override if non-empty
        if !other.extensions.is_empty() {
            self.extensions = other.extensions.clone();
        }
        if !other.skills.is_empty() {
            self.skills = other.skills.clone();
        }
        if !other.prompts.is_empty() {
            self.prompts = other.prompts.clone();
        }
        if !other.themes.is_empty() {
            self.themes = other.themes.clone();
        }
        if !other.enabled_models.is_empty() {
            self.enabled_models = other.enabled_models.clone();
        }
    }
}

/// Settings manager that handles loading from global and project locations
pub struct SettingsManager {
    global_path: PathBuf,
    project_path: PathBuf,
    settings: Settings,
}

impl SettingsManager {
    /// Create a new settings manager
    pub fn new(cwd: impl AsRef<Path>, config_dir: impl AsRef<Path>) -> Result<Self> {
        let global_path = config_dir.as_ref().join("settings.toml");
        let project_path = cwd.as_ref().join(".pi/settings.toml");

        // Load global settings
        let mut settings = Settings::from_file(&global_path)?;

        // Merge with project settings if they exist
        if project_path.exists() {
            let project_settings = Settings::from_file(&project_path)?;
            settings.merge(&project_settings);
        }

        Ok(Self {
            global_path,
            project_path,
            settings,
        })
    }

    /// Get the current settings
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Get mutable settings
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Save to global settings file
    pub fn save_global(&self) -> Result<()> {
        self.settings.save(&self.global_path)
    }

    /// Save to project settings file
    pub fn save_project(&self) -> Result<()> {
        self.settings.save(&self.project_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.theme, "dark");
        assert!(settings.compaction.enabled);
        assert_eq!(settings.autocomplete_max_visible, 5);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = Settings::default();
        let toml = toml::to_string(&settings).unwrap();
        let deserialized: Settings = toml::from_str(&toml).unwrap();
        assert_eq!(deserialized.theme, settings.theme);
    }

    #[test]
    fn test_settings_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let settings_path = temp_dir.path().join("settings.toml");

        let settings = Settings {
            theme: "light".to_string(),
            quiet_startup: true,
            ..Default::default()
        };

        settings.save(&settings_path).unwrap();
        let loaded = Settings::from_file(&settings_path).unwrap();

        assert_eq!(loaded.theme, "light");
        assert!(loaded.quiet_startup);
    }

    #[test]
    fn test_settings_merge() {
        let mut base = Settings::default();
        let override_settings = Settings {
            theme: "light".to_string(),
            quiet_startup: true,
            ..Default::default()
        };

        base.merge(&override_settings);

        assert_eq!(base.theme, "light");
        assert!(base.quiet_startup);
    }

    #[test]
    fn test_settings_merge_optional_fields() {
        let mut base = Settings::default();
        let override_settings = Settings {
            default_provider: Some("anthropic".to_string()),
            default_model: Some("claude-3".to_string()),
            shell_path: Some("/bin/zsh".to_string()),
            shell_command_prefix: Some("exec".to_string()),
            ..Default::default()
        };

        base.merge(&override_settings);

        assert_eq!(base.default_provider, Some("anthropic".to_string()));
        assert_eq!(base.default_model, Some("claude-3".to_string()));
        assert_eq!(base.shell_path, Some("/bin/zsh".to_string()));
        assert_eq!(base.shell_command_prefix, Some("exec".to_string()));
    }

    #[test]
    fn test_settings_merge_arrays() {
        let mut base = Settings::default();
        let override_settings = Settings {
            extensions: vec!["ext1".to_string(), "ext2".to_string()],
            skills: vec!["skill1".to_string()],
            prompts: vec!["prompt1".to_string()],
            themes: vec!["custom_theme".to_string()],
            enabled_models: vec!["model1".to_string(), "model2".to_string()],
            ..Default::default()
        };

        base.merge(&override_settings);

        assert_eq!(base.extensions, vec!["ext1", "ext2"]);
        assert_eq!(base.skills, vec!["skill1"]);
        assert_eq!(base.prompts, vec!["prompt1"]);
        assert_eq!(base.themes, vec!["custom_theme"]);
        assert_eq!(base.enabled_models, vec!["model1", "model2"]);
    }

    #[test]
    fn test_settings_merge_doesnt_override_none_optional_fields() {
        let mut base = Settings {
            default_provider: Some("existing".to_string()),
            shell_path: Some("/bin/bash".to_string()),
            ..Default::default()
        };
        let override_settings = Settings::default(); // All None

        base.merge(&override_settings);

        // Should keep existing values when override is None
        assert_eq!(base.default_provider, Some("existing".to_string()));
        assert_eq!(base.shell_path, Some("/bin/bash".to_string()));
    }

    #[test]
    fn test_settings_merge_doesnt_override_empty_arrays() {
        let mut base = Settings {
            extensions: vec!["ext1".to_string()],
            ..Default::default()
        };
        let override_settings = Settings::default(); // Empty arrays

        base.merge(&override_settings);

        // Should keep existing when override is empty
        assert_eq!(base.extensions, vec!["ext1"]);
    }

    #[test]
    fn test_settings_from_nonexistent_file() {
        let result = Settings::from_file("/nonexistent/settings.toml");
        assert!(result.is_ok());
        let settings = result.unwrap();
        assert_eq!(settings.theme, "dark"); // default
    }

    #[test]
    fn test_settings_manager() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();

        let manager = SettingsManager::new(&project_dir, &config_dir).unwrap();
        assert_eq!(manager.settings().theme, "dark"); // default
    }

    #[test]
    fn test_settings_manager_with_global_settings() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();

        // Write global settings
        let global_settings = Settings {
            theme: "light".to_string(),
            ..Default::default()
        };
        global_settings.save(config_dir.join("settings.toml")).unwrap();

        let manager = SettingsManager::new(&project_dir, &config_dir).unwrap();
        assert_eq!(manager.settings().theme, "light");
    }

    #[test]
    fn test_settings_manager_project_overrides_global() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(project_dir.join(".pi")).unwrap();

        // Write global settings
        let global_settings = Settings {
            theme: "light".to_string(),
            quiet_startup: false,
            ..Default::default()
        };
        global_settings.save(config_dir.join("settings.toml")).unwrap();

        // Write project settings that override
        let project_settings = Settings {
            theme: "monokai".to_string(),
            quiet_startup: true,
            ..Default::default()
        };
        project_settings.save(project_dir.join(".pi/settings.toml")).unwrap();

        let manager = SettingsManager::new(&project_dir, &config_dir).unwrap();
        assert_eq!(manager.settings().theme, "monokai");
        assert!(manager.settings().quiet_startup);
    }

    #[test]
    fn test_settings_manager_save() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(project_dir.join(".pi")).unwrap();

        let mut manager = SettingsManager::new(&project_dir, &config_dir).unwrap();
        manager.settings_mut().theme = "custom".to_string();
        manager.save_global().unwrap();
        manager.save_project().unwrap();

        // Reload and verify
        let manager2 = SettingsManager::new(&project_dir, &config_dir).unwrap();
        assert_eq!(manager2.settings().theme, "custom");
    }

    #[test]
    fn test_thinking_level_default() {
        let level = ThinkingLevel::default();
        assert_eq!(level, ThinkingLevel::Medium);
    }

    #[test]
    fn test_thinking_level_serialization() {
        let settings = Settings {
            default_thinking_level: ThinkingLevel::High,
            ..Default::default()
        };
        let toml = toml::to_string(&settings).unwrap();
        assert!(toml.contains("high"));
    }

    #[test]
    fn test_nested_settings_defaults() {
        let compaction = CompactionSettings::default();
        assert!(compaction.enabled);
        assert_eq!(compaction.reserve_tokens, 16384);

        let branch = BranchSummarySettings::default();
        assert_eq!(branch.reserve_tokens, 16384);

        let retry = RetrySettings::default();
        assert!(retry.enabled);
        assert_eq!(retry.max_retries, 3);

        let terminal = TerminalSettings::default();
        assert!(terminal.show_images);
        assert!(!terminal.clear_on_shrink);
    }
}
