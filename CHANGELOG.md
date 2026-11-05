# Changelog

All notable changes to **RustyBox** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [v1.3-57] - 2026-11-05
### Fixed & Improved (PR #2)
- **Fix Ctrl+C egui RwLock UI Freeze Deadlock**:
  - Tách luồng đọc `ui.input` trước khi xử lý phím, giải quyết triệt để lỗi `"unknown is not responding"` / UI freeze khi nhấn `Ctrl+C`.
  - Phân tách rõ ràng giữa Copy Text và `SIGINT` (`\x03`): Khi có vùng bôi đen thì `Ctrl+C` copy và xóa bôi đen; khi không có vùng chọn thì gửi ngay tín hiệu ngắt `SIGINT` cho các tiến trình con (`docker compose logs`, `ping`, `tail -f`).
  - Lọc nhiễu click chuột (< 5px) tránh việc vô tình tạo vùng chọn rỗng chặn `Ctrl+C`.
- **Terminal Focus Locking & Split-Pane Focus Routing**:
  - Khóa các phím `Tab`, `Escape`, `Arrow` vào terminal widget bằng `EventFilter`, chấm dứt việc nhấn `Tab` autocompletion trong shell làm nhảy focus ra ngoài giao diện.
  - Theo dõi chính xác tiêu điểm (active tab) khi chuyển đổi và click giữa các ô trong chế độ chia màn hình (`Split Horizontal`, `Vertical`, `2x2 Grid`).
- **Scrollback Navigation & Full History Search**:
  - Lưu trữ bộ đệm cuộn 2.000 dòng với thanh cuộn dọc tương tác bên phải và nhãn hiển thị vị trí cuộn trôi nổi.
  - Hỗ trợ cuộn qua chuột, phím `Shift+PageUp/PageDown`, `Shift+Up/Down`.
  - Bôi đen chọn text xuyên trang cuộn (`cross-page selection`) với cơ chế tự động cuộn ở biên trên/dưới.
  - Tìm kiếm toàn diện lịch sử terminal bằng phím `Ctrl+F` kèm nút điều hướng `▲ Prev` / `▼ Next`.
- **VT100 Emulation & SSH Capabilities**:
  - Hỗ trợ đầy đủ `DECCKM` (Application Cursor Mode `\x1bOA`..`\x1bOF`) phục vụ mượt mà cho `vim`, `less`, `htop`, `tmux`.
  - Hỗ trợ Bracketed Paste (`\x1b[200~...\x1b[201~`) bảo vệ việc dán mã lệnh nhiều dòng.
  - Chuẩn giao thức chuột TUI (`SGR 1006` / `X10`) và nhận diện URL `http://` / `https://` với `Ctrl+Click` để mở trình duyệt.
  - Render chuẩn ký tự độ rộng kép (Wide Characters / Emoji / CJK) và các thuộc tính text `bold`, `dim`, `inverse`, `underline`.
  - Kết nối toàn bộ tùy chọn cấu hình SSH nâng cao (`ssh_compression`, `tunnel_x11`, `ssh_keepalive`, `use_ssh_gateway`, `remote_command`, `log_terminal_output`) vào `PtySession`.

## [v1.3-56] - 2026-11-04
### Added
- **OpenSSH Profile Import (`~/.ssh/config`) (PR #1)**:
  - Phân tích cú pháp chỉ thị chuẩn OpenSSH (`Host`, `HostName` hỗ trợ mở rộng `%h`, `User`, `Port`, `IdentityFile`, `ProxyJump`, `ForwardX11`, `Compression`).
  - Hỗ trợ khối cấu hình mặc định toàn cục `Host *`, nhiều alias trên cùng một dòng, và thứ tự ưu tiên first-match.
  - Bổ sung nút bấm 1-click `🔑 Load ~/.ssh/config` trong hộp thoại Import và tự động nạp nội dung khi chọn tệp qua `📂 Browse...`.
  - Thêm phím tắt / nút bấm `📥 Import` trực quan trên Main Toolbar, Sidebar Sessions, hộp thoại New Session và Home Dashboard.
- **Home Dashboard: Recent Connections & Saved Sessions (PR #1)**:
  - Theo dõi thời điểm kết nối gần nhất (`last_connected_at`) và số lượt kết nối (`connect_count`) cho mỗi session (tương thích ngược hoàn toàn).
  - Tự động ghi nhận lịch sử kết nối khi khởi chạy bất kỳ terminal nào và tự động lưu phiên `Quick Connect`.
  - Thanh tìm kiếm thời gian thực `🔍 Filter Connections` hỗ trợ lọc theo tên, host, user, hoặc folder.
  - Mục **🕒 Recent Connections** hiển thị các máy chủ vừa kết nối (sắp xếp mới nhất lên đầu, kèm timestamp và số lần truy cập).
  - Mục **📂 Saved Sessions** hiển thị danh bạ máy chủ lưu trữ kèm context menu chuột phải (Connect, Edit, Clear from Recent, Delete).

## [v1.3-55] - 2026-11-04
### Added
- **Automated Stress Testing & Simulation Suite**:
  - Bổ sung bộ kiểm thử `tests/stress_user_simulation.rs` bao quát 100 chu kỳ cho Session CRUD, Vault AES-256-GCM / Argon2id, Credentials management và import bookmarks.
  - Xây dựng kịch bản kiểm thử GUI tự động `tests/qa_simulation_runner.py` trên Xvfb đo lường độ ổn định bộ nhớ và tương tác form.

### Fixed & Improved
- **New Session Dialog Validation**:
  - Bắt buộc kiểm tra Host/IP trước khi lưu session SSH/Telnet/RDP/VNC/FTP.
  - Hiển thị thông báo lỗi `⚠️` rõ ràng ngay trên modal thay vì im lặng đóng form hoặc sinh cấu hình rỗng.
- **Terminal Welcome Banner Fitting**:
  - Rút ngắn độ rộng dòng phân cách banner chào Local Terminal và SSH xuống ~50 ký tự, loại bỏ hoàn toàn hiện tượng vỡ dòng hoặc rớt chữ `UTF-8` khi chia đôi hoặc chia 4 màn hình (`2x2 Grid`).
- **Bottom Status & Telemetry Bar Layout**:
  - Phân vùng độc lập giữa thanh Quick Macros và System Telemetry; hỗ trợ cuộn ngang linh hoạt cho macros, ngăn chặn việc chip telemetry che khuất nút lệnh `clear`.
- **Dynamic Tab Width & Ellipsis**:
  - Tối ưu dải co giãn tab (100px - 180px) và thuật toán cắt chuỗi tự động theo độ rộng thực tế của từng tab.
- **Fault-Tolerant MobaXterm Parser**:
  - Hỗ trợ linh hoạt cả hai ký tự phân cách `#` và `%` trong định dạng bookmark của MobaXterm.

## [v1.3-54] - 2026-11-04
### Added
- **Auto-Prompt Master Password Vault on Startup**:
  - Tự động hiển thị hộp thoại mở khóa Vault ngay khi vừa khởi chạy ứng dụng nếu Vault đã được thiết lập và đang khóa.
  - Tự động đóng modal ngay sau khi mở khóa thành công, tránh việc đang kết nối SSH/SFTP mới bị popup chặn gây nghẽn luồng dữ liệu.

## [v1.3-53] - 2026-11-04
### Fixed
- **Windows ConPTY Handshake & SSH Login Hang**:
  - Xử lý phản hồi handshake con trỏ ban đầu (`\x1b[6n`) cho ConPTY trên Windows để ngăn chặn việc `ssh.exe` bị treo lúc khởi động.
  - Khắc phục lỗi không thể đăng nhập SSH và cải thiện luồng tự động điền mật khẩu phiên an toàn.

## [v1.3-52] - 2026-11-04
### Added
- **Real SFTP Remote File Explorer for VPS (`ssh2`)**: 
  - Thay thế toàn bộ file explorer cục bộ bằng SFTP Client bất đồng bộ thực tế qua thư viện `ssh2`.
  - Tự động kết nối SFTP và duyệt danh sách file trên VPS ngay khi người dùng mở phiên SSH (hỗ trợ cả mật khẩu và SSH private key).
  - Tự động chuyển đổi thư mục từ xa theo tab SSH đang active trên thanh công cụ.
  - Hỗ trợ đầy đủ các thao tác: duyệt thư mục từ xa, Upload từ local lên VPS, Download từ VPS về máy cục bộ, Chmod, Xóa file/folder và đo dung lượng đĩa thực tế của VPS (`df -k`).
  - Cung cấp nút chuyển đổi linh hoạt `💻 Local` và `● SFTP: <user@host>` trên thanh sidebar.

### Security
- **Eliminate SSH Auto-fill Password Leakage**: Thiết lập cơ chế timeout (10s) và nhận diện prompt thành công (`$ `, `# `, `Last login:`, `Welcome to`) để tự động hủy `pending_password`, ngăn chặn triệt để nguy cơ tự điền mật khẩu SSH vào shell khi chạy sudo hoặc xem file log.
- **Cryptographically Secure Password Generator**: Thay thế thuật toán LCG thô sơ bằng CSPRNG chuẩn (`OsRng`) từ hệ thống mật mã học.
- **Remove Fake SSH Key Generation**: Loại bỏ việc tạo key giả lập bằng UUID khi thiếu công cụ; thông báo rõ ràng yêu cầu cài đặt OpenSSH client (`ssh-keygen`).

### Performance & UX
- **Zero Every-Frame Disk I/O**: Loại bỏ việc đọc và parse file `RustyBox.json` trên đĩa ở mỗi khung hình UI, lưu cache trạng thái cấu hình Vault trong bộ nhớ.
- **Multi-line Paste Protection Toggle**: Tôn trọng cấu hình `paste_protection_enabled`, tự động paste thẳng vào terminal mà không bật popup cảnh báo khi người dùng đã tắt tùy chọn trong Settings.

## [v1.3-51] - 2026-11-04
### Fixed
- **Terminal SIGINT (Ctrl+C) Interrupt Reliability**: Bổ sung xử lý `Event::Copy` từ egui khi không có văn bản nào được chọn để gửi tín hiệu `\x03` (SIGINT) đến PTY process group thay vì nuốt phím. Khắc phục triệt để lỗi không thể `Ctrl+C` dừng lại khi xem luồng log dài hoặc liên tục (`docker compose logs`, `tail -f`, vòng lặp shell).
- **PTY Session Signal Handling**: Bổ sung phương thức `send_sigint()` gửi tín hiệu ngắt trực tiếp đến process con qua PTY master và system signal (`kill -2`), đảm bảo tiến trình foreground dừng ngay lập tức.
- **Cross-Platform Compatibility**: Hỗ trợ khởi chạy shell nội bộ Linux/Ubuntu trực tiếp khi click quick-launch WSL trên Linux.

## [v1.3-50] - 2026-11-04

### Fixed
- **Vietnamese Unicode Diacritics & Font Coverage**: Embedded native TTF fonts (`NotoSansMono`, `UbuntuMono`, `DejaVuSans`, `DejaVuSansMono`) into the executable. Resolves missing glyphs ("tofu" squares `□`) on Ubuntu / Linux for complex Vietnamese diacritical characters (`ễ`, `ọ`, `ạ`, `ờ`, `ậ`, `ộ`).
- **UI Font & Display Scaling**: Added dedicated UI Scale controls (`UI: ➖ 100% ➕`) with hotkeys (`Ctrl+Shift++`, `Ctrl+Shift+-`, `Ctrl+0`) and persistent storage in `data/RustyBox.json`.

---

## [v1.3-49] - 2026-11-04

### Added
- **Independent UI and Terminal Font Zoom**: Tách riêng biệt việc điều chỉnh kích thước hiển thị toàn bộ giao diện (UI Zoom) và kích thước chữ trong màn hình dòng lệnh (Terminal Font Zoom).

---

## [v1.3-48] - 2026-11-04

### Security
- **Embedded HTTP Server Path Traversal Protection**: Implemented strict canonical path resolution and prefix boundary verification (`canonical_target.starts_with(&canonical_root)`), preventing directory traversal attacks (`../`) and unauthorized host file access.
- **Credential Storage Hardening**: Enforced POSIX `0600` (read/write owner only) on `data/RustyBox.json` and `0700` on the `data/` directory to prevent local privilege snooping on multi-user systems.
- **Git Leak Prevention**: Added `/data` and `data/` to `.gitignore` to guarantee user credentials, private session hosts, and backups are never committed or pushed to public repositories.
- **Test Fixture Sanitization**: Replaced real IP references in unit test fixtures with RFC 5737 reserved testnet addresses (`198.51.100.42`).

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
