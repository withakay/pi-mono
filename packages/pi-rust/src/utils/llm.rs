// LLM client placeholder
//
// This module will house the HTTP client for LLM API calls (Anthropic, OpenAI, etc.)
// once LLM integration is implemented in a future phase.

/// Supported LLM providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmProvider {
    Anthropic,
    OpenAi,
    Custom(String),
}

/// Placeholder for an LLM response
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub stop_reason: Option<String>,
}

