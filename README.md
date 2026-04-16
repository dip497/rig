# Rig

A terminal UI for managing Claude AI skills and MCP servers.

## Features

- Manage AI skills in a modern TUI interface
- Configure and control MCP (Model Context Protocol) servers
- Project-based skill organization
- Interactive matrix view for skill/project relationships

## Installation

### One-line install (Linux & macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/dipendra-sharma/rig/main/install.sh | sh
```

Installs to `~/.local/bin/rig` by default. Override with `RIG_INSTALL_DIR`:

```bash
curl -fsSL https://raw.githubusercontent.com/dipendra-sharma/rig/main/install.sh | RIG_INSTALL_DIR=/usr/local/bin sh
```

### Download binary

Download from [GitHub Releases](https://github.com/dipendra-sharma/rig/releases):

| Platform | File |
|----------|------|
| Linux x86_64 | `rig-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `rig-aarch64-unknown-linux-musl.tar.gz` |
| macOS Intel | `rig-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `rig-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `rig-x86_64-pc-windows-msvc.zip` |

```bash
tar -xzf rig-x86_64-unknown-linux-musl.tar.gz
chmod +x rig
./rig
```

The Linux builds are statically linked (musl) — zero dependencies, works on any distro.

### Build from source

Requires Rust 1.70+:

```bash
cargo build --release
./target/release/rig
```

For a portable static Linux binary:

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
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
