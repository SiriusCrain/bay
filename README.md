# bay

IDE-style autocomplete for your terminal. Fork of the Amazon Q Developer CLI, scoped to local spec-driven autocomplete — no cloud, no AI, no auth.

## Features

- **Autocomplete** for hundreds of CLIs (`git`, `npm`, `docker`, `aws`, `kubectl`, ...) driven by the [withfig/autocomplete](https://github.com/withfig/autocomplete) spec set
- **Desktop popup** anchored to your cursor in the terminal, built on `tao` + `wry`
- **Shell integration** for bash, zsh, fish via a PTY shim (`figterm`)
- **IDE integration** for VSCode, JetBrains, Zed

## Installation

Build from source — see _Contributing_ below. A packaged AppImage is produced by `./build.sh --linux-packages appimage`.

## Contributing

### Prerequisites

Debian/Ubuntu:

```shell
sudo apt update
sudo apt install build-essential pkg-config jq dpkg curl wget cmake clang libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev valac libibus-1.0-dev libglib2.0-dev sqlite3 libxdo-dev protobuf-compiler
```

Rust toolchain via [rustup](https://rustup.rs):

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
rustup toolchain install nightly
cargo install typos-cli
```

macOS cross-targets:

```shell
rustup target add x86_64-apple-darwin aarch64-apple-darwin
```

Node and Python via [mise](https://mise.jdx.dev):

```shell
mise trust
mise install
```

Pre-commit hooks:

```shell
pnpm install --ignore-scripts
```

### Local development

Compile the CLI:

```shell
cargo run --bin q_cli
```

Append `-- <subcommand>` to run one, e.g. `cargo run --bin q_cli -- doctor`.

Run CLI tests:

```shell
cargo test -p q_cli
```

Format:

```shell
cargo +nightly fmt
```

Clippy:

```shell
cargo clippy --locked --workspace --color always -- -D warnings
```

## Project layout

- [`packages/autocomplete/`](packages/autocomplete/) — autocomplete React app
- [`packages/dashboard-app/`](packages/dashboard-app/) — dashboard React app
- [`crates/figterm/`](crates/figterm/) — PTY shim that intercepts the terminal's edit buffer
- [`crates/q_cli/`](crates/q_cli/) — the `q` CLI
- [`crates/fig_desktop/`](crates/fig_desktop/) — desktop shell (tao + wry)
- [`crates/fig_input_method/`](crates/fig_input_method/) — macOS input method for cursor position
- [`extensions/vscode/`](extensions/vscode/) — VSCode plugin
- [`extensions/jetbrains/`](extensions/jetbrains/) — JetBrains plugin
- [`build-scripts/`](build-scripts/) — Python build/sign orchestration
- [`proto/`](proto/) — protobuf IPC message definitions
- [`tests/`](tests/) — integration tests

See [`codebase-summary.md`](codebase-summary.md) for a summary of the Rust workspace crates.

## Licensing

Dual-licensed under MIT and Apache 2.0.
