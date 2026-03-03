# Pi Coding Agent - Rust Port

A Rust port of the pi coding agent, maintaining the same UX and logic structure while leveraging Rust's performance and safety.

## Status

🚧 **Work in Progress** — Core agent loop and tools are complete. LLM integration is the next milestone.

## Goals

- **Similar UX**: Keep UI, keybindings, and logic structure as close as possible to TypeScript version
- **Performance**: Faster startup and execution leveraging Rust
- **Safety**: Type-safe architecture preventing common bugs
- **Compatibility**: Read TypeScript session files (same JSONL format)

## What's Different from TypeScript Version

### Similarities
- Same UI layout (editor, messages, footer)
- Same keybindings and commands
- Same session format (JSONL)
- Same tool behavior (read, write, edit, bash, grep, find, ls)

### Differences
- Configuration in TOML instead of JSON (`~/.pi/rust-agent/`)
- Simplified hook system (no dynamic TypeScript extensions initially)
- ratatui-based TUI instead of custom Ink-like framework
- Faster startup (compiled binary, no Node.js runtime)
- Lower memory usage

### Not Included Initially
- Skills system (planned for future)
- Prompt templates (planned for future)
- Package manager for extensions
- OAuth authentication (API keys only initially)
- WASM plugin system (future phase)

## Building

```bash
cargo build --release
```

## Running

```bash
# Interactive REPL (stdin/stdout)
cargo run -- --session mysession

# Send a single message (print mode CLI)
cargo run -- --session mysession "Hello"

# Manage sessions
cargo run -- sessions          # list
cargo run -- new mysession     # create
cargo run -- info mysession    # details
cargo run -- delete mysession  # delete
```

## Development

### Prerequisites
- Rust 1.75+ (uses 2021 edition features)
- API key (Anthropic, once LLM integration lands)

### Testing

```bash
# Run all tests
cargo test

# Run with logging
RUST_LOG=debug cargo test
```

### Code Structure

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed architecture documentation.

```
src/
├── main.rs           # CLI entry point
├── lib.rs            # Library exports
├── core/             # Core agent logic
│   ├── messages.rs   # Message types & session entries
│   ├── events.rs     # Event bus (tokio broadcast)
│   ├── settings.rs   # TOML settings management
│   ├── persistence.rs# JSONL session storage
│   ├── session.rs    # Agent session state machine
│   ├── hooks.rs      # Hook system
│   └── compaction.rs # (stub) context compaction
├── tools/            # Built-in tools
│   ├── bash.rs       # Shell command execution
│   ├── read.rs       # File reading with pagination
│   ├── write.rs      # File writing
│   ├── edit.rs       # In-place text replacement + diff
│   ├── grep.rs       # Regex search respecting .gitignore
│   ├── find.rs       # Glob file/dir discovery
│   └── ls.rs         # Directory listing
├── modes/            # Execution modes
│   ├── interactive.rs# REPL over stdin/stdout
│   ├── print.rs      # Single-shot query
│   └── rpc.rs        # JSON-lines stdin/stdout protocol
├── ui/               # ratatui TUI components
│   ├── app.rs        # Full TUI event loop
│   ├── editor.rs     # Multi-line input widget
│   ├── messages.rs   # Scrollable message history
│   ├── footer.rs     # Status bar
│   ├── theme.rs      # Color palettes
│   └── keybindings.rs# Configurable key mappings
└── utils/            # Shared utilities
    ├── llm.rs        # (stub) LLM client types
    ├── paths.rs      # Path helpers
    └── truncate.rs   # Output truncation helpers
```

## Roadmap

- [x] Phase 1: Foundation & Setup
  - [x] Cargo workspace setup
  - [x] Architecture documentation
  - [x] Module structure
- [x] Phase 2: Core Domain Models
  - [x] Message types
  - [x] Event system
  - [x] Settings management
- [x] Phase 3: Tool System
  - [x] Bash executor
  - [x] File tools (read, write, edit)
  - [x] Search tools (grep, find, ls)
- [x] Phase 4: Agent Session Core
  - [x] State machine
  - [x] Session persistence
- [x] Phase 5: CLI
  - [x] Argument parsing
  - [x] Session management commands
- [x] Phase 6: Hook System
  - [x] Event dispatch
  - [x] Hook trait & registry
- [x] Phase 7: Additional Modes
  - [x] Print mode
  - [x] RPC mode
  - [x] Interactive REPL mode
- [x] Phase 8: UI Components
  - [x] Editor widget
  - [x] Messages panel
  - [x] Footer / status bar
  - [x] Theme system
  - [x] Keybinding system
  - [x] App event loop
- [ ] Phase 9: LLM API Integration
  - [ ] Anthropic API client
  - [ ] SSE streaming
  - [ ] Token counting & context management
- [ ] Phase 10: Advanced Features
  - [ ] Compaction logic
  - [ ] Session branching
  - [ ] End-to-end testing

## Contributing

This is a port of the TypeScript pi coding agent. Contributions should:
- Maintain compatibility with TypeScript session format
- Keep UX similar to TypeScript version
- Follow Rust best practices
- Include tests for new functionality

## License

MIT

## See Also

- [Pi Coding Agent (TypeScript)](../coding-agent/) - Original implementation
- [ARCHITECTURE.md](ARCHITECTURE.md) - Detailed architecture documentation
