# Rig

A terminal UI for managing Claude AI skills and MCP servers.

## Features

- Manage AI skills in a modern TUI interface
- Configure and control MCP (Model Context Protocol) servers
- Project-based skill organization
- Interactive matrix view for skill/project relationships

## Installation

### Download Pre-built Binary

Download the latest release for your platform from [GitHub Releases](https://github.com/dip497/rig/releases):

- **Linux**: `rig-linux-x86_64`
- **macOS Intel**: `rig-macos-x86_64`
- **macOS Apple Silicon**: `rig-macos-aarch64`
- **Windows**: `rig-windows-x86_64.exe`

Make it executable (Linux/macOS):
```bash
chmod +x rig-linux-x86_64
./rig-linux-x86_64
```

### Build from Source

Requires Rust 1.70+:

```bash
cargo build --release
./target/release/rig
```

## Usage

Launch the TUI:

```bash
rig
```

### Migrating from `npx skills`

If you have skills installed via `npx skills add`, migrate them into rig's store:

```bash
rig migrate
```

This will:
- Move skills from `~/.agents/skills/` → `~/.rig/skills/`
- Move config from `~/.config/rig/` → `~/.rig/`
- Scan agent directories for loose skills
- Create symlinks in all agent directories

### Navigation

- `Tab` - Switch between panels
- Arrow keys - Navigate lists
- `q` - Quit
- `?` - Help

## Configuration

All rig data lives in `~/.rig/`:

```
~/.rig/
├── config.json          # Settings, agents, projects
└── skills/              # Central skill store
    └── <skill-name>/
        └── SKILL.md
```

Skills are enabled per-agent via symlinks:
```
~/.claude/skills/<name>/  →  ~/.rig/skills/<name>/   (enabled)
```

## Development

### Build

```bash
cargo build
```

### Test

```bash
cargo test
```

### Lint

```bash
cargo clippy -- -D warnings
cargo fmt --check
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](LICENSE) for details.
