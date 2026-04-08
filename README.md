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

Run the application:

```bash
rig
```

### Navigation

- `Tab` - Switch between panels
- Arrow keys - Navigate lists
- `q` - Quit
- `?` - Help

## Configuration

Configuration files are stored in your system's config directory:
- Linux: `~/.config/rig/`
- macOS: `~/Library/Application Support/rig/`
- Windows: `%APPDATA%\rig\`

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
