// System prompt generation for the Pi coding agent

use chrono::Local;

/// Build the system prompt for the agent with context about the current environment.
pub fn build_system_prompt(cwd: &str) -> String {
    let date = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();

    format!(
        r#"You are Pi, an AI coding assistant running in a terminal.
You have access to tools to help with coding tasks:
- Read files with the read tool
- Write files with the write tool
- Edit files with the edit tool
- Search code with the grep tool
- Find files with the find tool
- List directories with the ls tool
- Run commands with the bash tool

Current working directory: {cwd}
Current date: {date}

When executing tasks, think step by step. Use tools as needed.
Always prefer reading existing code before making changes."#,
        cwd = cwd,
        date = date,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system_prompt_contains_cwd() {
        let prompt = build_system_prompt("/home/user/project");
        assert!(prompt.contains("/home/user/project"));
    }

    #[test]
    fn test_build_system_prompt_contains_date() {
        let prompt = build_system_prompt(".");
        // The date field should be present and formatted as YYYY-MM-DD
        assert!(prompt.contains("Current date:"));
        // Verify the date format includes a 4-digit year
        let re = regex::Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
        assert!(re.is_match(&prompt), "Prompt should contain a date in YYYY-MM-DD format");
    }

    #[test]
    fn test_build_system_prompt_mentions_tools() {
        let prompt = build_system_prompt(".");
        assert!(prompt.contains("read tool"));
        assert!(prompt.contains("write tool"));
        assert!(prompt.contains("bash tool"));
    }
}
