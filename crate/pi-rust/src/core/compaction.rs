use crate::core::messages::{Message, MessageContent, MessageRole};

/// Maximum number of messages to include in a compaction summary
const MAX_SUMMARY_MESSAGES: usize = 10;

/// Estimate token count for a string.
/// Uses a rough approximation of 4 characters per token; actual tokenization
/// varies by model and may differ significantly for code or non-English text.
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() / 4).max(1)
}

/// Estimate tokens for a message
pub fn estimate_message_tokens(msg: &Message) -> usize {
    match &msg.content {
        MessageContent::Text(t) => estimate_tokens(t) + 4,
        MessageContent::Blocks(blocks) => {
            blocks
                .iter()
                .map(|b| match b {
                    crate::core::messages::ContentBlock::Text { text } => estimate_tokens(text),
                    crate::core::messages::ContentBlock::ToolUse { input, name, .. } => {
                        estimate_tokens(name) + estimate_tokens(&input.to_string())
                    }
                    crate::core::messages::ContentBlock::ToolResult { content, .. } => {
                        estimate_tokens(content)
                    }
                    crate::core::messages::ContentBlock::Thinking { thinking } => {
                        estimate_tokens(thinking)
                    }
                })
                .sum::<usize>()
                + 4
        }
    }
}

/// Check if messages exceed a token limit
pub fn exceeds_limit(messages: &[&Message], limit: usize) -> bool {
    messages
        .iter()
        .map(|m| estimate_message_tokens(m))
        .sum::<usize>()
        > limit
}

/// Compact messages by keeping system messages and recent context,
/// summarizing older messages. Returns remaining messages.
/// Simple strategy: keep first N (system/context) and last M messages
pub fn compact_messages(
    messages: Vec<Message>,
    max_tokens: usize,
    keep_recent: usize,
) -> (Vec<Message>, String) {
    let msgs_ref: Vec<&Message> = messages.iter().collect();
    if !exceeds_limit(&msgs_ref, max_tokens) {
        return (messages, String::new());
    }

    let recent_start = messages.len().saturating_sub(keep_recent);
    let (older, recent) = messages.split_at(recent_start);

    let summary = build_summary(older);

    (recent.to_vec(), summary)
}

/// Build a text summary of messages
fn build_summary(messages: &[Message]) -> String {
    if messages.is_empty() {
        return String::new();
    }

    let mut parts = Vec::new();
    parts.push(format!("Summary of {} earlier messages:", messages.len()));

    for msg in messages.iter().take(MAX_SUMMARY_MESSAGES) {
        if let Some(text) = msg.text_content() {
            let preview = if text.len() > 200 {
                format!("{}...", &text[..200])
            } else {
                text.to_string()
            };
            let role = match msg.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
                MessageRole::System => "System",
            };
            parts.push(format!("[{}]: {}", role, preview));
        }
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::messages::Message;

    #[test]
    fn test_estimate_tokens() {
        assert!(estimate_tokens("hello world") > 0);
        assert!(estimate_tokens("") >= 1);
    }

    #[test]
    fn test_exceeds_limit() {
        let msg = Message::user("hello");
        let msgs = vec![&msg];
        assert!(!exceeds_limit(&msgs, 10000));
    }

    #[test]
    fn test_compact_keeps_recent() {
        let messages: Vec<Message> = (0..10)
            .map(|i| Message::user(format!("message {}", i)))
            .collect();

        let (compacted, summary) = compact_messages(messages, 10, 3);
        assert_eq!(compacted.len(), 3);
        assert!(!summary.is_empty());
    }

    #[test]
    fn test_compact_no_change_when_within_limit() {
        let messages: Vec<Message> = (0..3)
            .map(|i| Message::user(format!("msg {}", i)))
            .collect();

        let (compacted, summary) = compact_messages(messages.clone(), 100000, 5);
        assert_eq!(compacted.len(), messages.len());
        assert!(summary.is_empty());
    }
}
