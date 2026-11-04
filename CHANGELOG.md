# Changelog

All notable changes to **RustyBox** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [v1.3-47] - 2026-11-04

### Fixed
- **UI Tab Bar Alignment**: Moved the `Home` tab directly inside the scrollable tab row container (`ScrollArea`), eliminating the 6px vertical offset and achieving sub-pixel baseline alignment (`Y=78..105`, height `28px`) matching all adjacent terminal tabs.
- **Sidebar Tab Layout**: Unified `📂 Sessions` and `📁 SFTP Browser` into a horizontal segmented control on a single line.
- **Clean Deployment**: Removed redundant root configuration files from the distribution directory so that config is strictly managed within `./data/RustyBox.json`.

---

## [v1.3-45] - 2026-11-04

### Added
- **Modern Dark UI Aesthetic**: Redesigned color palette using deep Slate/Zinc tones (`#0A0E17`, `#10141D`), high-contrast text (`#F0F6FC`), subtle glowing borders (`#222A3A`), and consistent 6px–8px rounded geometry.
- **Balanced 3x2 Action Cards**: Standardized Home Welcome Dashboard with 6 quick-access cards (*Local Terminal*, *SSH Client*, *SFTP Browser*, *Network Scanner*, *SSH Tunnels*, *Network Daemons*) featuring colored icon badges and smooth hover effects.
- **System Telemetry Chips**: Integrated bottom telemetry indicators (*Host, User, CPU load & cores, RAM usage gauge, Net I/O, Uptime*) alongside quick-command shortcuts.
- **Context-Aware Quick Connect**: Quick connect capsule in the tab bar now dynamically displays only when working inside terminal tabs to prevent screen redundancy.

---

## [v1.3-44] - 2026-11-04

### Added
- **Argon2id + AES-256-GCM Master Password Vault**:
  - Implemented military-grade credential protection using Argon2id key derivation with a 16-byte random hardware salt.
  - Authenticated encryption (AEAD) with AES-256-GCM and unique 12-byte random nonces per field.
  - Zero-plaintext storage: all session passwords and shared credentials on disk (`data/RustyBox.json`) are stored strictly as `enc:v1:<nonce>:<ciphertext>`.
  - On-Demand Lazy Lock architecture: safe in-memory decryption for PTY auto-login without writing plain secrets to temporary files.
- **Auto-Backup & Self-Healing Storage**:
  - Automatic snapshot generation (`data/RustyBox.json.<unixtime>`) with rolling retention capped at the 3 newest backups.
  - Self-healing recovery: automatically detects corrupted JSON on startup and restores from the newest valid backup.

---

## [v1.3-0] - 2026-10-31

### Added
- **Multi-Split View**: Single, 2-Columns (Horizontal), 2-Rows (Vertical), and 2x2 Grid multi-terminal views.
- **Multi-Exec Broadcast**: One-click broadcast switch to mirror keystrokes across all open terminal tabs simultaneously.
- **Graphical Dual-Pane SFTP Explorer**: Upload, download, create, delete, and inline file editor with auto-follow session tracking.
- **Embedded Network Tools**: Subnet IP scanner, asynchronous TCP port scanner, DNS lookup, and Wake-on-LAN.
- **SSH Tunnels Manager**: Local, remote, and dynamic (SOCKS5) port forwarding with live status toggles.
- **Integrated Utilities**: SSH Key Generator (RSA 2048/4096, Ed25519), Password Generator with entropy rules, Text Diff viewer, and MobApt package explorer.
- **Cross-Platform PTY & ConPTY**: Native Windows ConPTY support via `portable-pty` and VT100 / Xterm 256-color / 24-bit TrueColor rendering.
