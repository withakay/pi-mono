# Pi Coding Agent - Rust Port Architecture

This document describes the architecture of the Rust port of the pi coding agent, mapping TypeScript components to Rust equivalents.

## Design Goals

1. **Similar UX**: Keep UI, keybindings, and logic structure as close as possible to TypeScript version
2. **Performance**: Leverage Rust's performance for faster startup and tool execution
3. **Safety**: Use Rust's type system to prevent common bugs
4. **Modularity**: Clean separation of concerns with well-defined interfaces
5. **Extensibility**: Support hooks and plugins (simplified from TypeScript extensions)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     PI CODING AGENT (Rust)                      │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────── CLI LAYER ───────────────────────────────┐
│ Argument Parsing (clap) → Config → Session Selection            │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────── CORE LAYER ─────────────────────────────┐
│                                                                  │
│  ┌────────────────┐   ┌──────────────┐   ┌─────────────────┐   │
│  │ Event System   │   │ Hook System  │   │ Session State   │   │
│  │ (mpsc channels)│   │              │   │                 │   │
│  │                │   │ - Lifecycle  │   │ - Conversation  │   │
│  │ - Subscribe    │   │ - Tool hooks │   │   History       │   │
│  │ - Dispatch     │   │              │   │ - Compaction    │   │
│  │ - Serialize    │   │              │   │                 │   │
│  └────────────────┘   └──────────────┘   └─────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Agent Session (Core State Machine)                      │   │
│  │                                                         │   │
│  │ 1. Build context (recent msgs + compaction)           │   │
│  │ 2. Call LLM (streaming via reqwest)                   │   │
│  │ 3. Parse response (thinking + tools + text)           │   │
│  │ 4. Execute tools (with hook interception)             │   │
│  │ 5. Loop until no more tool calls                      │   │
│  │ 6. Save to session & emit events                      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
    ┌─────────────────────────┼─────────────────────────┐
    │                         │                         │
    ▼                         ▼                         ▼
┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│ INTERACTIVE  │      │     RPC      │      │    PRINT     │
│     MODE     │      │     MODE     │      │     MODE     │
│ (ratatui)    │      │ (JSON I/O)   │      │ (single-shot)│
└──────────────┘      └──────────────┘      └──────────────┘
```

## Module Structure

```
src/
├── main.rs              # Entry point, CLI argument handling
├── lib.rs               # Library exports for SDK usage
├── core/
│   ├── mod.rs           # Core module exports
│   ├── session.rs       # AgentSession state machine
│   ├── messages.rs      # Message types and structures
│   ├── events.rs        # Event system (mpsc channels)
│   ├── hooks.rs         # Hook system for extensibility
│   ├── compaction.rs    # Session compaction logic
│   ├── persistence.rs   # Session save/load (JSONL)
│   └── settings.rs      # Configuration management
├── tools/
│   ├── mod.rs           # Tool trait and registry
│   ├── bash.rs          # Bash execution with streaming
│   ├── read.rs          # File reading
│   ├── write.rs         # File writing
│   ├── edit.rs          # File editing with diff
│   ├── grep.rs          # Pattern search
│   ├── find.rs          # File discovery
│   ├── ls.rs            # Directory listing
│   └── executor.rs      # Common tool execution logic
├── modes/
│   ├── mod.rs           # Mode trait
│   ├── interactive.rs   # Interactive TUI mode
│   ├── rpc.rs           # RPC mode (JSON stdin/stdout)
│   └── print.rs         # Print mode (single-shot)
├── ui/
│   ├── mod.rs           # UI components
│   ├── app.rs           # Main TUI application state
│   ├── editor.rs        # Input editor component
│   ├── messages.rs      # Message display component
│   ├── footer.rs        # Status footer
│   ├── theme.rs         # Theme support
│   └── keybindings.rs   # Keyboard shortcut handling
├── cli/
│   ├── mod.rs           # CLI parsing and setup
│   ├── args.rs          # Command-line arguments (clap)
│   └── config.rs        # Config file loading
└── utils/
    ├── mod.rs           # Utility exports
    ├── llm.rs           # LLM API client
    ├── truncate.rs      # Output truncation
    └── paths.rs         # Path utilities
```

## Key Design Decisions

### 1. Event System: mpsc Channels vs TypeScript EventEmitter

**TypeScript approach:**
- EventEmitter pattern with string-based event names
- Synchronous and async event handlers
- Extensions subscribe to events

**Rust approach:**
- Use tokio::sync::mpsc channels for event dispatch
- Type-safe event enums instead of strings
- Subscribers receive Arc'd events

```rust
pub enum AgentEvent {
    SessionStart { session_id: String },
    MessageStart { message_id: String },
    ToolCall { tool: String, input: Value },
    ToolResult { tool: String, output: String },
    // ... more variants
}

// Subscribers get a channel receiver
let mut rx = agent.subscribe();
while let Some(event) = rx.recv().await {
    match event {
        AgentEvent::ToolCall { tool, input } => { /* ... */ }
        // ...
    }
}
```

### 2. Extension/Hook System: Simplified from TypeScript

**TypeScript approach:**
- Full TypeScript module loading via jiti
- Extensions can register tools, commands, UI components
- Complex API surface with many capabilities

**Rust approach (Phase 1 - Hooks):**
- Built-in hook points at key lifecycle events
- Hooks are Rust trait implementations
- No dynamic loading initially (compiled-in hooks)

```rust
#[async_trait]
pub trait Hook: Send + Sync {
    async fn on_tool_call(&self, tool: &str, input: &Value) -> Result<()> {
        Ok(())
    }

    async fn on_session_start(&self, session_id: &str) -> Result<()> {
        Ok(())
    }

    // ... more hook methods
}
```

**Future Phase - WASM Plugins:**
- Load extensions as WASM modules
- More restrictive API than TypeScript (for safety)
- Still provides core extension points

### 3. Session Persistence: JSONL (same as TypeScript)

Keep the same JSONL format for session files to maintain compatibility:

```jsonl
{"id":"1","parentId":null,"role":"user","content":"Hello"}
{"id":"2","parentId":"1","role":"assistant","content":"Hi there!"}
```

This allows:
- Reading TypeScript sessions in Rust
- Session format compatibility
- Append-only writes (no file rewriting)

### 4. TUI: ratatui vs Ink-like (TypeScript uses @mariozechner/pi-tui)

**TypeScript approach:**
- Custom Ink-like framework (pi-tui)
- Component-based with React-like patterns
- 4400+ lines for interactive mode

**Rust approach:**
- Use ratatui (mature Rust TUI library)
- Simpler component model
- Focus on core features first

Key components:
- Editor (multi-line input with @ file references)
- Messages (streaming assistant responses)
- Footer (status, tokens, model)
- Dialogs (model selection, settings)

### 5. Tool Execution: Async with Streaming

**TypeScript approach:**
- Async/await with streaming via Node.js streams
- BashExecutor manages process lifecycle
- Output truncation and ANSI sanitization

**Rust approach:**
- async/await with tokio
- async-process for command execution
- tokio::sync::mpsc for output streaming

```rust
pub struct BashExecutor {
    // ...
}

impl BashExecutor {
    pub async fn execute(
        &self,
        command: &str,
    ) -> Result<impl Stream<Item = String>> {
        // Spawn process, return stream of output lines
    }
}
```

### 6. LLM Integration: HTTP Client vs pi-ai Library

**TypeScript approach:**
- @mariozechner/pi-ai library (unified multi-provider API)
- Streaming SSE/WebSocket support
- Provider abstraction

**Rust approach (Phase 1):**
- Direct HTTP API calls via reqwest
- Start with Anthropic API
- Parse SSE streams manually

**Future:**
- Create Rust equivalent of pi-ai
- Multi-provider support
- Or use FFI to call TypeScript pi-ai

### 7. Configuration: TOML vs JSON

**TypeScript approach:**
- JSON for settings, models, sessions
- ~/.pi/agent/settings.json

**Rust approach:**
- TOML for settings (more Rusty, comments supported)
- ~/.pi/rust-agent/settings.toml
- Keep JSONL for sessions (compatibility)

## Differences from TypeScript Version

### What's the Same
- UI layout (editor, messages, footer)
- Keybindings (same shortcuts)
- Session format (JSONL)
- Tool behavior (read, write, edit, bash, grep, find, ls)
- Compaction logic
- Session branching
- Core agent loop (prompt → LLM → tools → repeat)

### What's Different
- No dynamic TypeScript extension loading (initially)
- Simpler hook system instead of full extension API
- Configuration in TOML instead of JSON
- Using ratatui instead of custom Ink-like framework
- Direct API calls instead of pi-ai library (initially)
- Compiled binary (faster startup, no Node.js runtime)

### What's Not Included (Initially)
- Skills system (can add later)
- Prompt templates (can add later)
- Theme hot-reloading (can add later)
- Package manager (npm/git installs)
- OAuth authentication (API keys only initially)
- Full extension system (WASM plugins in future)

## Implementation Phases

See main README for phase breakdown. Key principle:
**Build incrementally with TDD, small commits, continuous testing**

## Performance Targets

- Startup time: < 100ms (vs ~500ms for TypeScript)
- Tool execution: Similar or faster than TypeScript
- Memory usage: Lower than Node.js version
- Binary size: < 20MB (release build with optimizations)

## Compatibility

- Can read TypeScript session files (same JSONL format)
- Cannot load TypeScript extensions (different runtime)
- Settings are separate (~/.pi/rust-agent vs ~/.pi/agent)

## Testing Strategy

1. **Unit tests**: Each module has tests in `tests/` dir
2. **Integration tests**: Test full workflows
3. **TDD approach**: Write tests before implementation
4. **Comparison tests**: Run same prompts in TS and Rust, compare outputs
