# Pi Coding Agent (Rust)

Rust implementation of the pi coding agent, maintaining the same UX and logic structure while leveraging Rust's performance and safety.

## Status

🚧 **Work in Progress** - Early development phase

Current focus: Core domain models and tool system

## Goals

- **Similar UX**: Keep UI, keybindings, and logic structure consistent across implementations
- **Performance**: Faster startup and execution leveraging Rust
- **Safety**: Type-safe architecture preventing common bugs
- **Compatibility**: Read and write the same JSONL session format

## Key Characteristics

### Core behavior
- Same UI layout (editor, messages, footer)
- Same keybindings and commands
- Same session format (JSONL)
- Same tool behavior (read, write, edit, bash, grep, find, ls)
- Same core agent loop and logic

### Runtime and architecture
- Configuration in TOML (~/.pi/rust-agent/)
- Simplified hook system (no dynamic runtime extensions initially)
- ratatui-based TUI instead of custom Ink-like framework
- Faster startup (compiled binary)
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
# From source
cargo run

# From binary
./target/release/pi
```

## Development

### Prerequisites
- Rust 1.75+ (uses 2021 edition features)
- API key (Anthropic initially)

### Testing

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test --test session_tests

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
├── tools/            # Built-in tools
├── modes/            # Interactive, RPC, Print modes
├── ui/               # TUI components
├── cli/              # CLI argument parsing
└── utils/            # Shared utilities
```

## Roadmap

- [x] Phase 1: Foundation & Setup
  - [x] Cargo workspace setup
  - [x] Architecture documentation
  - [x] Module structure
- [ ] Phase 2: Core Domain Models
  - [ ] Message types
  - [ ] Event system
  - [ ] Settings management
- [ ] Phase 3: Tool System
  - [ ] Bash executor
  - [ ] File tools (read, write, edit)
  - [ ] Search tools (grep, find, ls)
- [ ] Phase 4: Agent Session Core
  - [ ] State machine
  - [ ] Session persistence
  - [ ] Compaction logic
- [ ] Phase 5: Hook System
  - [ ] Event dispatch
  - [ ] Hook trait
- [ ] Phase 6: Interactive TUI
  - [ ] Editor
  - [ ] Message display
  - [ ] Footer/status
- [ ] Phase 7: Additional Modes
  - [ ] Print mode
  - [ ] RPC mode
- [ ] Phase 8: Integration & Polish
  - [ ] LLM API integration
  - [ ] Theme support
  - [ ] End-to-end testing

## Contributing

Contributions should:
- Maintain compatibility with the JSONL session format
- Keep UX consistent with the project conventions
- Follow Rust best practices
- Include tests for new functionality

## License

MIT

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) - Detailed architecture documentation
