# Contributing to RustyBox

Thank you for your interest in contributing to **RustyBox**! We welcome bug reports, feature requests, documentation improvements, and code contributions.

---

## 🚀 Getting Started

### Prerequisites
- **Rust Toolchain:** Stable Rust (edition 2024, Rust 1.85+ recommended).
- **Cargo:** Bundled with Rust via `rustup`.
- **Operating System:**
  - **Linux:** `build-essential`, `libx11-dev`, `libxcb1-dev`, `libxcursor-dev`, `libxkbcommon-dev`.
  - **Windows:** MSVC toolchain or MinGW-w64 (`x86_64-pc-windows-gnu`).
  - **macOS:** Standard Xcode Command Line Tools.

### Cloning & Building Locally
```bash
git clone https://github.com/longkaka/rust-remote-toolbox.git
cd rust-remote-toolbox

# Run in debug mode
cargo run

# Run tests
cargo test

# Build optimized release binary
cargo build --release
```

---

## 🧪 Testing & Quality Assurance

RustyBox adheres to strict automated testing standards:
1. **Unit & Storage Tests:**
   ```bash
   cargo test
   ```
   All tests in `src/session/storage.rs`, `src/app.rs`, and `src/terminal/` must pass 100% without failures or regressions.
2. **Code Cleanliness:**
   ```bash
   cargo check
   cargo clippy --all-targets --all-features
   ```
3. **Headless GUI Testing (Linux / WSL):**
   GUI features can be exercised headlessly using Xvfb:
   ```bash
   Xvfb :99 -screen 0 1280x800x24 &
   DISPLAY=:99 LIBGL_ALWAYS_SOFTWARE=1 cargo run
   ```

---

## 📐 Project Architecture

- **`src/main.rs`**: Application entry point, window lifecycle configuration via `eframe`.
- **`src/app.rs`**: Main application state (`RustyBoxApp`), UI layout, theme visuals, Action Cards, and dialog handlers.
- **`src/session/`**:
  - `storage.rs`: Portable configuration persistence, rolling 3-backup retention, JSON corruption auto-recovery, and Argon2id + AES-256-GCM Vault security.
  - `ui.rs`: Session tree navigation, credential dialogs, and context menus.
  - `import_export.rs`: MobaXterm session format parser and JSON interchange.
- **`src/terminal/`**:
  - `pty_session.rs`: Cross-platform ConPTY/PTY spawn, background I/O, and runtime credential injection.
  - `vt_screen.rs`: VT100 / ANSI escape sequence parsing and high-performance font rendering.
- **`src/sftp/`**: Dual-pane remote file manager and inline text editor.
- **`src/network_tools/`**: Async TCP port scanning, ping utility, and DNS lookup.
- **`src/tunneling/`**: SSH port forwarding and SOCKS5 proxy managers.

---

## 🔒 Security Guidelines

- Never commit plaintext credentials or SSH private keys.
- Ensure all sensitive credential stores continue to pass through `prepare_for_disk` and encrypt with AES-256-GCM when the Master Vault is active.
- Sensitive credentials must remain strictly in memory (`RAM-only`) and must never be dumped to logs, terminal output, or temporary files.

---

## 📝 Pull Request Guidelines

1. Create a feature branch: `git checkout -b feat/your-feature-name`.
2. Follow Rust idiomatic formatting (`cargo fmt`).
3. Ensure all 9 unit tests pass (`cargo test`).
4. Commit with descriptive, conventional commit messages:
   - `feat: ...` for new features
   - `fix: ...` for bug fixes
   - `docs: ...` for documentation
5. Submit a Pull Request with a clear summary of changes and verification steps.
