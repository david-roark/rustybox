# Changelog

All notable changes to **RustyBox** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [v1.4.3] - 2026-11-16
### Changed
- Merge PR #10: refresh the RustyBox logo and synchronize PNG, ICO and embedded RGBA assets.
- Merge PR #11: remove stale backup code and unused dependencies, reuse the library from the binary entry point, narrow Tokio features and clean up compiler/Clippy warnings.
- Merge PR #12: cache Rust dependencies, run Windows/Linux tests before release builds, make PTY tests platform-aware, enable symbol stripping and ThinLTO, and separate release permissions from build jobs.
- Keep macOS GUI screenshots in Actions artifacts rather than public release downloads.
### Known behavior
- Recursive folder uploads skip symbolic links, including links to files. Regular files and real subdirectories are traversed; symlink targets are not copied. This behavior was reviewed and accepted for this merge.

## [v1.4.2] - 2026-11-13
### Fixed & Improved (Smooth Startup & Accurate SFTP Disconnect Detection)
- **Sửa triệt để lỗi popup "⚠️ Disconnected · 🔄 Reconnect" tự động hiện khi đang loading**:
  - Bổ sung hàm kiểm tra chính xác `is_confirmed_disconnected()`: Chỉ xác nhận mất kết nối khi thỏa mãn đồng thời: phiên là remote, không đang trong quá trình tải (`!is_loading`), đã ngắt kết nối (`!is_connected`) và có thông báo lỗi ngắt kết nối socket/fatal error từ server.
  - Tuyệt đối không bao giờ hiển thị popup/badge mất kết nối khi đang kết nối, đang refresh hoặc tải thư mục rỗng.
  - Chấm tròn trạng thái trên thanh SFTP: chuyển màu vàng cam khi đang loading/connecting, xanh lá khi kết nối bình thường, và chỉ chuyển đỏ khi có lỗi ngắt kết nối thực sự.
- **Khắc phục hiện tượng chớp hình khi khởi động ứng dụng trên Windows**:
  - Triển khai `clear_color` trong `eframe::App` đồng bộ chuẩn màu nền Dark `#0F121A` (100% opacity), loại bỏ việc clear canvas sang màu bán trong suốt mặc định ở frame đầu tiên.
  - Tối ưu lệnh phóng to cửa sổ `Maximized`: Bỏ qua việc gửi lệnh `ShowWindow(SW_MAXIMIZE)` trùng lặp khi cửa sổ đã được Windows khởi tạo ở trạng thái maximized (`viewport().maximized == Some(true)`), ngăn chặn hiện tượng DWM vẽ lại khung viền gây chớp màn hình.
  - Khởi tạo tỷ lệ thu phóng `set_zoom_factor(ui_scale)` ngay trong hàm khởi dựng `new(cc)` và chỉ cập nhật lại khi người dùng thay đổi tỷ lệ thay vì gọi gán lại liên tục mỗi frame.

## [v1.4.1] - 2026-11-13
### Fixed & Improved (SFTP Reconnection & Session-43 FXP_OPEN Error)
- **Khắc phục triệt để lỗi `[Session(-43)] Unable to send FXP_OPEN*` khi reboot server**:
  - Tự động phát hiện lỗi ngắt kết nối kênh SSH / socket disconnect (`is_fatal_ssh_error`) trong luồng SFTP worker và giải phóng phiên chết ngay lập tức thay vì giữ lại phiên lỗi.
  - Bổ sung lệnh `SftpCommand::ForceReconnect` và kiểm tra tính hợp lệ sống còn của channel trước khi tái sử dụng session trong `Connect`.
  - Cải tiến hàm `refresh()`: Tự động nhận diện nếu SFTP đang mất kết nối thì kích hoạt tái kết nối đầy đủ (`force_reconnect_remote`) thay vì cố đọc thư mục trên socket hỏng.
  - Bổ sung nút **`🔄 Reconnect`** trực quan ngay tại thanh thông báo lỗi SFTP và banner thông báo khi bảng file trống do mất kết nối.
  - Tự động tái kết nối SFTP khi chuyển tab hoặc khi restart tab terminal sau khi server reboot xong.

## [v1.4.0] - 2026-11-13
### Added & Improved (UI Ergonomics, SFTP Resizable Columns & Auto-Reconnect)
- **Tối ưu hóa hộp thoại Master Password**:
  - Đổi cơ chế hiển thị sang `.pivot(CENTER_CENTER).default_pos(center)`, tăng kích thước `500x340 px`.
  - Tăng kích thước nút `Close` lên `90x26 px` và canh lề phải cân đối, hài hòa chuẩn UI.
- **Tối ưu hóa cảnh báo Multi-line Paste Warning**:
  - Cho phép resize cửa sổ cảnh báo dán nhiều dòng.
  - Thêm nút tiện ích **`📋 Copy Text`** trên thanh công cụ và menu chuột phải `📋 Copy` ngay trong khung soạn thảo để copy nhanh nội dung lệnh mà không cần bôi đen phức tạp.
- **Bổ sung Copy trên thanh đường dẫn SFTP**:
  - Hỗ trợ click chuột phải (Context Menu) vào ô đường dẫn SFTP hiển thị tùy chọn **`📋 Copy Path`** tiện lợi.
- **Kéo thả co giãn độ rộng cột Name trong SFTP**:
  - Bổ sung Resizer (`col_name_width_override`) tại vạch phân cách giữa cột `Name` và `Size`.
  - Cho phép người dùng rê chuột kéo giãn tùy thích khi tên file quá dài; nhấp đúp (Double Click) để tự động reset về độ rộng mặc định.
- **Tự động kết nối lại SFTP khi Terminal khởi động lại (Reboot / Restart)**:
  - Khi tab SSH được kích hoạt khởi động lại qua phím `Enter` hoặc menu context, phiên SFTP tương ứng sẽ tự động tái kết nối nền mà không cần đóng mở lại toàn bộ ứng dụng.

## [v1.3.99] - 2026-11-12
### Fixed & Improved (MobaXterm 95-Version Changelog Audit & System Hardening)
- **Khắc phục xung đột phím Escape khi đóng Modal/Dialog**:
  - Tự động bỏ khóa focus phím Escape của terminal và chặn phát byte `\x1b` khi có bất kỳ cửa sổ Popup/Modal nào đang mở (Settings, Manage Passwords, SFTP dialogs, v.v.), tránh làm ảnh hưởng đến shell hay thoát ứng dụng TUI bên trong terminal ngoài ý muốn.
- **Làm sạch khoảng trắng trường Ghi chú (Comments)**:
  - Tự động `.trim()` chuỗi ghi chú khi mở chỉnh sửa và khi lưu session, chống hiện tượng tích tụ khoảng trắng ở cuối chuỗi qua nhiều lần biên tập.
- **Khử trùng lặp tên Session không phân biệt hoa/thường**:
  - Chuẩn hóa bộ khóa nhận diện trùng lặp trong `merge_imported_sessions` sang dạng `.to_lowercase()` cho tất cả các loại session phi SSH (Local Terminal, WSL, Serial, v.v.).
- **Chống treo giao diện khi Paste Clipboard**:
  - Dùng `try_lock_for(Duration::from_millis(200))` cho clipboard toàn cục, tránh kẹt luồng UI nếu ứng dụng ngoài đang giữ khóa Win32 Clipboard.
- **Bảo toàn Startup Macro**:
  - Thêm trường `startup_macro` vào model `Session`, hỗ trợ nạp macro khi duplicate và tự động kích hoạt macro khi kết nối tab SSH / terminal cục bộ.
- **Làm sạch địa chỉ IP / Hostname khi Paste**:
  - Lọc bỏ triệt để ký tự `\r`, `\n`, `\t` khi copy/paste địa chỉ máy chủ, username hoặc jump host từ các ứng dụng chat.
- **Chặn ký tự ngoặc trong tên Profile Credential**:
  - Lọc bỏ các ký tự ngoặc `[]`, `{}` để đảm bảo tính toàn vẹn cho cấu trúc lưu trữ và serialize INI/JSON.
- **Reset trạng thái tìm kiếm trong Code Editor**:
  - Đóng thanh tìm kiếm và xóa query cũ khi mở file văn bản mới qua SFTP.

## [v1.3.98] - 2026-11-10
### Added & Improved (Kibi Modular Code Editor, Sudo Batch Deletion & Remote SFTP Path Normalization)
- **Tích hợp Kibi-inspired Modular Code Editor (PR #9)**:
  - Tách trình chỉnh sửa văn bản thành module độc lập (`src/editor/`), hỗ trợ tô màu cú pháp (Syntax Highlighting) chuyên nghiệp cho 18+ ngôn ngữ lập trình và cấu hình: Shell/Bash, YAML, JSON, TOML, Python, Rust, C/C++, SQL, HTML/XML, JavaScript/TypeScript, Go, PHP, CSS, Lua, Ruby, Java/C#, v.v.
  - Hỗ trợ cú pháp nhúng đa ngữ cảnh trong HTML (JS trong `<script>`, CSS trong `<style>`, PHP trong `<?php ?>`, Ruby/ERB trong `<% %>`).
  - Thước căn thụt dòng thông minh (Visual Indent Guides `┊`), nút công cụ `🧹 Clean Tabs` chuyển đổi nhanh tab thành khoảng trắng.
  - Tìm kiếm từ khóa mạnh mẽ (`Ctrl + F`, `F3`, `Shift + F3`), zoom kích thước chữ, ngắt dòng (word wrap), và trạng thái lưu file an toàn chống mất dữ liệu.
  - Tự động nhận diện và chặn mở các tệp nhị phân / tệp nén (`.gz`, `.zip`, `.tar`, ELF) để tránh lỗi UTF-8 hoặc tràn bộ nhớ.
- **Khắc phục triệt để xóa file trong chế độ Sudo SFTP (Root Mode)**:
  - Chuẩn hóa phân cách đường dẫn Windows `\` thành `/` POSIX khi gửi lệnh sang server Linux từ máy client Windows, khắc phục hiện tượng lệnh `rm -rf` không tìm thấy file.
  - Thêm lệnh `DeleteMany` xử lý xóa hàng loạt đồng thời cho các file được bôi chuột quét chọn (marquee / multi-selection), loại bỏ race condition và refresh thừa.
  - Bắt trọn vẹn thông báo lỗi `stderr` của sudo và tự động focus ô nhập mật khẩu Sudo Modal.
- **Tự động nhận diện OS của Server từ xa (`RemoteOsKind::Posix` vs `Windows`)**:
  - Chuẩn hóa đường dẫn hiển thị, chuyển thư mục cha (`go_up`), và danh mục Quick Jump phù hợp theo hệ điều hành của máy chủ từ xa.
- **Bổ sung nút Copy Mật khẩu trong Password Settings & An toàn thao tác**:
  - Thêm nút copy nhanh `📋` gọi trực tiếp Win32 clipboard API (`arboard`), tự động giải mã AES-256-GCM từ Vault trước khi copy.
  - Tách khoảng cách an toàn cho nút xóa `🗑` màu đỏ nhạt, tránh nguy cơ click xóa nhầm khi thao tác copy.
  - Tăng kích thước mặc định form Password Settings (`740x580`), mở khóa hoàn toàn resize/move tự do (`pivot + default_pos`), danh sách cuộn co giãn linh hoạt theo kích thước cửa sổ.
- **Nâng phiên bản**:
  - Tăng số hiệu phiên bản lên **`v1.3.98`** tích hợp hoàn chỉnh PR #9 và các bản vá SFTP/Vault.

---

## [v1.3.96] - 2026-11-09
### Added & Improved (Sudo SFTP Mode, Elevated File Management & Root Built-in Editor)
- **Tính năng 🛡️ Sudo SFTP Mode (Root Mode)**:
  - Cho phép người dùng duyệt cây thư mục bị hạn chế quyền (như `/root`, `/etc/ssl/private`, `/var/log`), xem, sửa, tạo, xóa, đổi tên và phân quyền file hệ thống dưới quyền root thông qua kênh thực thi ngầm `sudo -S`.
  - **Built-in Editor hỗ trợ quyền Root**: Đọc và lưu file trực tiếp qua kênh Sudo (Base64 encode / decode qua `sudo tee`), giải quyết triệt để lỗi `Permission Denied` khi chỉnh sửa các tệp cấu hình hệ thống quan trọng (`/etc/nginx/nginx.conf`, `/etc/hosts`, v.v.).
  - **Nút chuyển đổi chủ động (Toggle Button) trên SFTP Toolbar**:
    - Trạng thái mặc định: Nút xám tối `🛡️ Sudo SFTP` (chạy theo quyền user SSH thông thường).
    - Trạng thái kích hoạt: Đổi màu cam nổi bật `🛡️ Sudo Active` kèm biểu tượng khiên sáng để người dùng luôn nhận biết rõ mình đang thao tác với quyền Root tối cao.
  - **Hộp thoại xác thực mật khẩu Sudo Modal**: Khi máy chủ đòi hỏi mật khẩu sudo và phiên kết nối chưa có sẵn (ví dụ đăng nhập bằng SSH Key), ứng dụng tự động hiển thị popup căn giữa màn hình yêu cầu nhập mật khẩu và hỗ trợ ghi nhớ trong suốt phiên làm việc.
- **Cải thiện độ rộng thanh Sidebar mặc định**:
  - Tăng độ rộng khởi tạo mặc định của Left Sidebar (Sessions / SFTP Explorer) từ `260px` lên `280px` giúp hiển thị tên tệp/thư mục và địa chỉ host dài trực quan, thoáng đãng hơn.
- **Quy chuẩn định dạng phiên bản mới**:
  - Chuyển đổi định dạng phiên bản từ dấu gạch nối (`v1.3-95`) sang chuẩn dấu chấm (`v1.3.96`), tên file thực thi Windows xuất bản là `RustyBox-v1.3.96.exe`.

---

## [v1.3-95] - 2026-11-09
### Fixed & Improved (Universal Dark Mode Enforcement across OS Appearances & Standalone macOS Builds)
- **Khóa cứng Dark Mode trên toàn bộ nền tảng (macOS, Windows, Linux)**:
  - Khắc phục triệt để lỗi vùng làm việc trung tâm (`CentralPanel`) và các ô nhập liệu (`Quick Connect`, tìm kiếm) bị màu trắng trên macOS runner hoặc khi hệ điều hành đang bật Light Mode.
  - Thiết lập cứng `egui::ThemePreference::Dark` và áp dụng bộ visuals tối đồng bộ cho cả `Theme::Dark` và `Theme::Light` trong `eframe`, đảm bảo giao diện luôn giữ tone màu xanh đen sâu (`#0F121A`) đồng nhất, chữ sáng rõ ràng và không bao giờ bị chói mắt.
  - Gán màu nền tường minh cho khung làm việc trung tâm (`central_panel`) và ngăn chặn tràn lấn layout của thanh telemetry dưới đáy.
- **Tích hợp OpenSSL Vendored độc lập cho macOS (`ssh2 vendored-openssl`)**:
  - Nhúng tĩnh thư viện OpenSSL vào binary thực thi của macOS (`aarch64-apple-darwin` và `x86_64-apple-darwin`), giúp ứng dụng chạy độc lập (standalone portable) trên macOS mà không cần cài đặt OpenSSL hoặc Homebrew.
- **Sửa nhãn hiển thị phiên bản**:
  - Cập nhật đồng bộ nhãn phiên bản trên thanh công cụ và hộp thoại About thành `v1.3-95`.

---

## [v1.3-94] - 2026-11-08
### Fixed & Improved (Native macOS Compatibility, Cmd vs Ctrl Keybindings, Telemetry & GitHub Actions macOS Runner)
- **Tương thích hoàn chỉnh với hệ điều hành macOS (Apple Silicon & Intel)**:
  - **Phím tắt `Cmd` vs `Control` chuẩn macOS (`src/terminal/widget.rs`):** Tách riêng phím `Command (⌘)` và `Control (⌃)` trên macOS: `Cmd + C` chỉ sao chép vùng chọn (không bao giờ gửi nhầm `SIGINT \x03` làm ngắt tiến trình đang chạy khi chưa chọn văn bản), `Cmd + V` dán từ Clipboard, `Cmd + K` xóa màn hình, trong khi `Ctrl + C/D/Z/L/R` giữ nguyên là phím điều khiển Terminal thực thụ.
  - **Khởi tạo Login Shell & đường dẫn Homebrew (`src/terminal/pty_session.rs`):** Tự động mở `/bin/zsh -l` và bổ sung `/opt/homebrew/bin`, `/usr/local/bin` vào `PATH` để ứng dụng chạy từ Finder vẫn gọi đầy đủ các công cụ hệ thống.
  - **Tự động xử lý Gatekeeper AppTranslocation (`src/session/storage.rs`):** Tự động lưu cấu hình vào `~/Library/Application Support/RustyBox/data` nếu ứng dụng được mở từ ổ đĩa ảo chỉ đọc (`.dmg` hoặc `/AppTranslocation/`).
  - **Đồng bộ CWD & Telemetry trên macOS (`src/app.rs`, `src/telemetry/mod.rs`):** Sử dụng `lsof`, `sysctl`, `vm_stat` và `scutil` để hiển thị đầy đủ Hostname, CPU, RAM, Uptime và đồng bộ thư mục hiện tại của Local Terminal trên macOS.
  - **GitHub Actions macOS CI/CD (`.github/workflows/macos.yml`):** Tự động biên dịch, chạy bộ kiểm thử và khởi chạy GUI trên máy chủ macOS 14 (Apple Silicon).

---

## [v1.3-93] - 2026-11-08
### Added & Improved (SFTP Multi-Select Marquee Drag & Centered Modal Confirmation Popups)
- **Hỗ trợ chọn nhiều tệp/thư mục trong SFTP Explorer (Multi-Select)**:
  - **Kéo quét chuột (Rubber-band / Marquee Selection):** Nhấn giữ chuột trái và kéo quét dọc danh sách hoặc từ vùng trống bên dưới để tô khung xanh chọn hàng loạt tệp/thư mục cùng lúc.
  - **Phím tắt tiêu chuẩn:** Hỗ trợ đầy đủ `Ctrl + Click` (chọn/bỏ chọn lẻ), `Shift + Click` (chọn theo dải liền mạch), `Ctrl + A` (chọn tất cả), và phím `Delete` (mở hộp thoại xóa hàng loạt).
  - **Thao tác hàng loạt (Batch Actions):** Menu chuột phải và thanh công cụ SFTP hỗ trợ **Download hàng loạt**, **Delete hàng loạt**, **Copy toàn bộ đường dẫn**, và **Paste danh sách đường dẫn vào Terminal**.
- **Chuyển toàn bộ hộp thoại xác nhận & nhập liệu SFTP sang Popup giữa màn hình**:
  - Loại bỏ các thanh hỏi nhỏ chèn ở phía trên sidebar SFTP, thay bằng các cửa sổ Modal Popup căn giữa màn hình rõ ràng, trực quan:
    - **⚠️ Xác nhận xóa (`Confirm Delete`):** Hiển thị rõ tên tệp hoặc danh sách cuộn các mục khi xóa nhiều mục cùng lúc; hỗ trợ phím `Enter` để xác nhận và `Esc` để hủy.
    - **⚠️ Xử lý trùng tên tệp (`File Already Exists`):** Cửa sổ Popup nổi bật khi Upload/Download trùng tên với 3 nút: `🔄 Ghi đè (Overwrite)`, `📝 Đổi tên (Keep both)`, `⏭ Bỏ qua (Skip)`.
    - **📁 Tạo thư mục mới / 📄 Tạo tệp mới:** Chuyển sang cửa sổ Popup nhập tên trực quan, hỗ trợ nhấn `Enter` tạo nhanh.

---

## [v1.3-92] - 2026-11-08
### Fixed & Improved (Built-in Editor Auto-Centering, Fixed 80% Height & SSH Login Default to Terminal + SFTP)
- **Cố định chiều cao Editor Built-in chuẩn 80% màn hình & triệt tiêu lỗi phình chiều cao khi co kéo**:
  - Khóa cứng chiều cao cửa sổ ở đúng **80% chiều cao màn hình** (`screen.height() * 0.80`), căn giữa hoàn hảo (`pivot(CENTER_CENTER)`).
  - Khi người dùng kéo co hẹp chiều rộng cửa sổ ("kéo ngắn lại"), chiều cao cửa sổ vẫn giữ nguyên cố định 80%, không bao giờ bị phình dài ra hay tràn mép dưới màn hình.
  - Người dùng có thể tự do co giãn chiều ngang (từ 420px đến gần trọn màn hình) rất mượt mà.
- **Tự động chuyển đến Terminal và mở SFTP Explorer tương ứng khi đăng nhập SSH thành công**:
  - Khi phiên SSH được khởi tạo và kết nối thành công, ứng dụng tự động hiển thị tab Terminal của phiên đó đồng thời chuyển thanh điều hướng trái sang tab **SFTP Explorer** (`sidebar_tab = 1`) bám sát đúng máy chủ từ xa.
  - Tự động bắt sự kiện `SftpEvent::Connected` từ tiến trình nền để đảm bảo cây thư mục SFTP của VPS hiển thị tức thời ngay khi xác thực thành công.
- **Cập nhật thông tin tác giả**:
  - Cập nhật hiển thị tên tác giả theo nickname: `Long kakaka (longanhvn)` trong hộp thoại About và `README.md`.

---

## [v1.3-91] - 2026-11-08
### Fixed & Improved (Multi-Tab Management, Sessions Auto-Switch, SFTP Editor Overhaul & About Repo Link)
- **Quản lý đa tab thông minh khi mở 10–20+ Tab cùng lúc**:
  - Tích hợp thanh cuộn ngang `ScrollArea::horizontal` kèm cơ chế tự động cuộn đến tab đang chọn (`auto-scroll active tab into view`).
  - Bổ sung **Menu chuyển tab nhanh (`📑 N ▾` Tab Switcher)** trực tiếp trên thanh công cụ: hiển thị danh sách toàn bộ tab kèm biểu tượng nhận diện (Local, SSH, WSL, Telnet), hỗ trợ chuyển đổi hoặc đóng tab riêng lẻ tức thì, kèm lối tắt "Đóng tất cả tab không hoạt động" và "Mở Local Terminal".
- **Tự động chuyển Sidebar về tab Sessions khi mở Tab mới**:
  - Mọi thao tác khởi tạo tab mới (Local Terminal, kết nối SSH từ danh bạ hoặc Quick Connect) đều tự động chuyển thanh điều hướng trái về tab **Sessions**, giúp người dùng luôn quản lý danh bạ máy chủ thuận tiện.
- **Tối ưu hóa toàn diện Built-in Code/Text Editor SFTP (Chống tràn giao diện)**:
  - Khắc phục triệt để lỗi tràn khung nhìn khi mở các tệp có nội dung lớn hoặc dòng văn bản siêu dài (log nén, JSON một dòng).
  - Cửa sổ hỗ trợ co giãn tự do (`resizable`), kích thước mặc định 860x580, tối thiểu 520x360.
  - Bổ sung nút bấm **Bật/Tắt bẻ dòng (`↩ Wrap: ON / OFF`)**: tự động gói dòng theo chiều rộng hiển thị mà không làm phình cửa sổ.
  - Cố định thanh trạng thái chân trang (hiển thị số dòng, dung lượng tệp, đường dẫn remote) luôn nằm ngay ngắn ở đáy cửa sổ, không bao giờ bị đẩy mất.
  - Cột số dòng (Line Numbers Gutter) tính toán độ rộng chuẩn xác theo cỡ chữ và căn phải chuyên nghiệp.
  - Cuộn hai chiều mượt mà (`ScrollArea::both`) cho cả dòng văn bản dài và file nhiều trang.
- **Cập nhật Hộp thoại About RustyBox**:
  - Thêm liên kết trực tiếp đến GitHub Repository chính thức: `https://github.com/longanhvn/rustybox`.
  - Cập nhật số hiệu phiên bản `v1.3-91` và danh mục tính năng hoàn chỉnh.
- **Làm mới toàn bộ tài liệu `README.md`**:
  - Đồng bộ đầy đủ các tính năng hiện tại của ứng dụng.
  - Cập nhật chính xác địa chỉ repository và số lượng kiểm thử tự động (160/160 unit tests).
- **Triệt tiêu hiện tượng chớp màn hình đen / cửa sổ console conhost khi mở app**:
  - Chuyển cơ chế khởi tạo Terminal nội bộ sang Lazy Spawn: Không tự động chạy ngầm phiên ConPTY shell (`Local Terminal`) lúc vừa mở ứng dụng, ngăn chặn hoàn toàn hệ điều hành Windows bật chớp cửa sổ `conhost.exe` ngầm.
  - Thiết lập cấu hình Viewport `with_transparent(false)` và tối ưu hóa chu kỳ vẽ frame đầu tiên của `eframe`/`glow`, loại bỏ hiện tượng giật tối khung hình khi GPU khởi tạo.
- **Tối ưu hóa tối đa chiều rộng thanh Tab Bar**:
  - Loại bỏ hoàn toàn các thành phần trang trí không cần thiết bên phải thanh tab, nhường toàn bộ không gian ngang cho dải tab terminal.
  - Tích hợp nút menu chuyển tab nhanh `📑 N ▾` xuất hiện thông minh khi có từ 2 tab trở lên.

---

## [v1.3-89] - 2026-11-07
### Added & Improved (SFTP Speed Optimization, Advanced Internal Editor, Auto Session Logging & SFTP Bookmarks)
- **Tối ưu hóa băng thông SFTP Transfer (512KB Buffer & 2MB Socket)**:
  - Tăng dung lượng bộ đệm `BufReader` / `BufWriter` và mảng buffer truyền tải SFTP từ 256KB lên 512KB.
  - Tăng kích thước bộ đệm socket TCP cấp hệ thống (`set_send_buffer_size` & `set_recv_buffer_size`) lên 2MB kết hợp cờ `TCP_NODELAY = true`, tối ưu hóa tối đa thông lượng upload/download trên kết nối VPS băng thông cao và độ trễ lớn.
- **Tự động chuyển Sidebar về tab Sessions khi đóng Tab Terminal**:
  - Khi đóng tab (qua phím tắt `Ctrl+Shift+W`, nút `X`, menu chuột phải hoặc thanh tác vụ), ứng dụng tự động chuyển `sidebar_tab = 0` (Sessions Tree) giúp người dùng ngay lập tức thấy danh sách máy chủ/phiên để tiếp tục kết nối.
- **Tự động ghi Log Terminal ra tệp theo ngày và xoay vòng 5 ngày (Session Logging to File)**:
  - Thư mục log động chuẩn hóa theo thư mục dữ liệu ứng dụng: `<data_dir>/logs/` (mặc định đặt ngay cạnh file cấu hình `data/logs/`, hỗ trợ cả chế độ portable).
  - Phân loại thư mục con theo từng ngày (`YYYY-MM-DD/`).
  - Cơ chế tự động dọn dẹp (auto-prune): Chỉ giữ lại tối đa 5 ngày gần nhất, các thư mục log cũ hơn 5 ngày sẽ được tự động xóa sạch để giải phóng dung lượng đĩa.
  - Bổ sung nút bấm `📂 Open Logs` trong cài đặt để mở nhanh thư mục log bằng Windows File Explorer.
  - Tự động đồng bộ và lưu trữ cấu hình vào `RustyBox.json`.
  - Bổ sung cột số dòng (Line Numbers) tự động căn chỉnh và đồng bộ theo cỡ chữ.
  - Hỗ trợ phím tắt `Ctrl + S` lưu ngay lập tức (ghi đè và tự upload lên VPS), phím tắt `Ctrl + F` mở thanh tìm kiếm từ khóa kèm đếm số lượt khớp (match count).
  - Nút thu phóng cỡ chữ nhanh (`➖` / `➕`) từ 9px đến 28px.
  - Thanh trạng thái chân trang hiển thị chi tiết số dòng, độ dài bytes và đường dẫn tệp đầy đủ.
- **Trình biên tập tệp từ xa tích hợp nâng cao (Built-in Code/Text Editor)**:
- **SFTP Bookmarks & Quick Jump thư mục yêu thích**:
  - Bổ sung nút ⭐ Bookmark ngay trên thanh Path bar SFTP để lưu nhanh/xóa thư mục hiện tại.
  - Danh mục `⭐ Bookmarked Directories` hiển thị trực tiếp trong menu popup dropdown `[▾]` kèm nút xóa 🗑️.
  - Đồng bộ và lưu trữ danh sách Bookmarks vào `RustyBox.json`.

---

## [v1.3-88] - 2026-11-07
### Added & Improved (Merge PR #7 & PR #8 from david-roark)
- **Tích hợp PR #7: Đo đạc Telemetry hệ thống từ xa thời gian thực qua SSH Worker**:
  - Bổ sung module `src/telemetry/mod.rs` thu thập số liệu chuẩn xác: CPU %, Cores, RAM Used/Total (GB & %), Root Disk %, Real-time Net Upload/Download rate (▲/▼), và Uptime từ các tệp `/proc/stat`, `/proc/meminfo`, `/proc/net/dev`, `df -k /`, `uptime -p`.
  - Thay thế hoàn toàn cơ chế regex quét màn hình terminal và mockup cũ; hiển thị an toàn ký tự `--` khi chưa có dữ liệu hoặc phiên ngắt kết nối.
  - SSH background worker định kỳ thăm dò telemetry mỗi 2.5s khi rảnh và tự động ngắt thăm dò khi đang upload/download tệp để bảo vệ băng thông mạng.
- **Tích hợp PR #8: Tùy biến tùy chọn Copy-on-select trên Terminal**:
  - Thêm cấu hình `copy_on_select` (mặc định `true`) vào `RustyBoxConfig` và lưu trữ bền vững vào `RustyBox.json`.
  - Thêm checkbox bật/tắt trong `Settings & Preferences -> Terminal`: `Automatically copy selected text to clipboard (Copy on select)`.
  - Trên Linux: khi tắt copy-on-select, vùng bôi đen chỉ cập nhật `PRIMARY` selection (hỗ trợ paste chuột giữa) mà không ghi đè `CLIPBOARD` hệ thống (Ctrl+V); trên Windows: chỉ bôi đen trực quan mà không chạm vào bộ nhớ tạm.

---

## [v1.3-87] - 2026-11-07
### Fixed & Improved (Full Codebase Stability Audit: Lifecycle Cleanup, POSIX SFTP Paths & Config Persistence)
- **Thu hồi triệt để tiến trình nền và kết nối khi thoát ứng dụng (`eframe::App::on_exit`)**:
  - Triển khai `on_exit` và `shutdown_cleanup()` để tự động dừng toàn bộ SSH Tunnels (`stop_all_tunnels`), tắt 3 máy chủ nhúng (`HTTP`, `Syslog`, `TFTP`), ngắt mọi phiên SFTP từ xa (`disconnect_all_remotes`) và kill sạch toàn bộ tiến trình PTY đang mở.
  - Thay thế `std::process::exit(0)` ở `Sessions -> Exit` bằng `ViewportCommand::Close` để đi qua luồng thu hồi tài nguyên chuẩn; bổ sung cờ `/T` cho `taskkill /F /T /PID` (kèm `CREATE_NO_WINDOW`) trong `TunnelManager` trên Windows để diệt cả cây tiến trình con.
- **Chuẩn hóa đường dẫn SFTP POSIX `/` trên Windows & Hàng đợi xung đột Download**:
  - Khắc phục lỗi chèn dấu `\` của `Path::join` trên Windows làm hỏng tên tệp/thư mục trên máy chủ Linux từ xa trong `create_folder`, `create_file`, `rename_entry`, `execute_download`, `resolve_conflict_overwrite` và `resolve_conflict_rename`; chặn ký tự `/`, `\` và `..` trong tên đối tượng mới.
  - Bổ sung hàng đợi `pending_conflicts` cho `download_file` đồng bộ với chiều Upload / Drag & Drop.
- **Lưu trữ bền vững & Sao lưu/Khôi phục toàn diện cấu hình (`RustyBox.json`)**:
  - Lưu xuống đĩa và tự động khôi phục khi khởi động các tùy chọn: `quick_macros`, `password_save_mode`, `save_key_passphrases`, `remote_monitoring_enabled`, `follow_terminal_folder`, và `font_size` (khi Zoom qua Menu, Context Menu hoặc phím tắt).
  - Đồng bộ `Export` / `Import` ở cả thanh Menu `Sessions` lẫn cửa sổ `Settings` để sao lưu và nạp trọn vẹn toàn bộ Sessions, Credentials, Snippets, Tunnels, Macros, Highlight Rules, giao diện và tùy chọn SFTP.
- **Đồng bộ thanh Menu `Terminal` & Bảng phím tắt**:
  - Đồng bộ hành vi `Terminal -> Close Active Tab` và `Terminal -> Duplicate Current Tab` hoạt động nhất quán với thanh Tab (dọn tiến trình, kế thừa phiên SSH/Local, đồng bộ Sidebar SFTP) và cập nhật bảng tra cứu `Keyboard Shortcuts`.

---

## [v1.3-86] - 2026-11-07
### Added & Improved (Native OS Drag-and-Drop to SFTP Upload)
- **Kéo thả File & Folder từ Windows Explorer vào SFTP (`Drag & Drop Upload`)**:
  - Bật cờ `ViewportBuilder::with_drag_and_drop(true)` để lắng nghe sự kiện kéo thả tệp tin hệ điều hành trên Windows/Linux.
  - Tự động chuyển sang tab SFTP nếu người dùng đang ở tab Sessions mà kéo tệp lướt qua thanh Sidebar khi đang kết nối VPS.
  - Hiển thị khung viền sáng xanh neon kèm Overlay trực quan (`📥 Drop N item(s) to Upload` + đường dẫn thư mục đích) khi kéo tệp vào bảng SFTP.
  - Tự động phân loại File lẻ và Thư mục đệ quy (`handle_dropped_paths`), hỗ trợ hàng đợi xử lý xung đột nhiều tệp trùng tên liên tiếp (`pending_conflicts`) và mở rộng chiều cao khung danh sách SFTP sát đáy sidebar.

---

## [v1.3-85] - 2026-11-06
### Fixed (Windows Process Cleanup & Console Window Suppression)
- **Triệt tiêu hiện tượng nháy cửa sổ dòng lệnh đen khi tắt tab / thoát ứng dụng**:
  - Bổ sung cờ `CREATE_NO_WINDOW = 0x08000000` thông qua `std::os::windows::process::CommandExt` cho toàn bộ các lệnh gọi `taskkill.exe` (trong dọn dẹp PTY Session và SSH Tunnels).
  - Áp dụng cờ ẩn cửa sổ `CREATE_NO_WINDOW` cho `cmd /C start` (mở tệp mặc định) và `ssh-keygen` khi tạo cặp khóa SSH trên Windows.
  - Đảm bảo toàn bộ quá trình thu hồi tài nguyên và diệt tiến trình con chạy 100% ngầm trong nền, không làm chớp/hiện bất kỳ cửa sổ console `conhost.exe` nào ra màn hình.

---

## [v1.3-84] - 2026-11-06
### Added & Improved (Merge PR #5 & PR #6 from duyvuchatai)
- **Tích hợp PR #5: Bộ chọn văn bản Terminal, Clipboard Wayland & Cross-Platform Tests**:
  - Hỗ trợ `GLOBAL_CLIPBOARD` và tính năng `wayland-data-control` trên Linux, giữ kết nối clipboard/primary selection không bị mất khi drop object.
  - Phân luồng sao chép và dán (`copy_text_to_clipboard`, `get_clipboard_text`): Tự động hỗ trợ Linux Primary selection (cho thao tác chuột giữa và Shift+Insert) và Windows standard clipboard.
  - Tinh chỉnh bôi đen văn bản bằng chuột: Thêm cờ `!just_stopped_drag` ngăn kích hoạt nhầm sự kiện click/double-click khi vừa thả chuột sau khi drag; hỗ trợ dọn dẹp bộ nhớ tạm `clear_selection_in_ui`.
  - Thuật toán nhận diện biên từ thông minh (`find_word_bounds_in_row`): Hỗ trợ double-click chọn trọn vẹn toàn bộ URL (`https://...`), địa chỉ IP, đường dẫn tệp (`/var/log/...`).
  - Bổ sung 6 bộ unit test tự động kiểm thử toàn diện: kéo chuột xuôi/ngược, bôi đen nhiều dòng, phím tắt sao chép (Ctrl+Shift+C, Ctrl+Insert, Event::Copy) và kiểm tra giải phóng vùng chọn khi click đơn.
- **Tích hợp PR #6: Đăng ký ứng dụng Desktop & Icon chuẩn cho Linux Dock**:
  - Tự động tạo tệp desktop launcher `~/.local/share/applications/rustybox.desktop` và icon chuẩn 8-bit RGBA `rustybox.png`.
  - Bổ sung định danh `.with_app_id("rustybox")` cho ViewportBuilder, giúp hệ thống cửa sổ Linux/Wayland ghim đúng icon trên Taskbar/Dock.

---

## [v1.3-83] - 2026-11-06
### Added & Improved (SFTP Recursive Folder Transfer & MobaXterm Top Controls)
- **Tải lên & Tải xuống đệ quy cả thư mục (Recursive Folder Transfer)**:
  - Bổ sung menu ngữ cảnh chuột phải: `📤 Upload file(s)...`, `📁 Upload folder...`, `📥 Download folder`.
  - Hỗ trợ chọn thư mục nội bộ qua `rfd::FileDialog` và tự động tạo cây thư mục đệ quy cả hai chiều upload và download qua SFTP.
- **Tối ưu tốc độ truyền tải SFTP (Speed Optimization)**:
  - Cấu hình TCP socket qua `socket2`: Kích hoạt `TCP_NODELAY = true`, mở rộng buffer gửi/nhận socket hệ thống lên **1MB** (`1024 * 1024`).
  - Nâng kích thước buffer đọc/ghi luồng SFTP lên **256KB** bằng `BufReader` và `BufWriter`, gia tăng đáng kể thông lượng truyền file lớn.
- **Tái cấu trúc thanh điều khiển SFTP chuẩn MobaXterm**:
  - Đưa cụm nút **Remote monitoring** (kèm icon ECG monitor) và checkbox **Follow terminal folder** lên vị trí đầu trang ngay bên dưới thanh địa chỉ (Path bar), phía trên danh sách file, không còn bị khuất ở chân trang khi danh sách tệp dài.
  - Tự động kích hoạt hiển thị tab SFTP Browser ngay khi mở phiên kết nối SSH.

---

## [v1.3-82] - 2026-11-06
### Fixed & Improved (Terminal Selection & Quick Copy)
- **Cải tiến bôi đen kéo chuột & sao chép tức thời phong cách MobaXterm**:
  - Chuẩn hóa tọa độ click và kéo chuột trong widget terminal (`pos_to_abs_cell`), hỗ trợ bôi đen liên tục qua các trang cuộn scrollback.
  - Thao tác nhanh: Double-click bôi đen và sao chép từ/token, Triple-click bôi đen và sao chép toàn bộ dòng lệnh.
  - Tự động sao chép văn bản vào clipboard ngay khi nhả chuột (Select-to-copy) và xóa vùng chọn khi click chuột trái đơn lẻ hoặc khi người dùng gõ phím.

---

## [v1.3-81] - 2026-11-06
### Added & Improved (Branding & Application Icon)
- **Cập nhật Icon 3D Isometric Network Cube to sát viền**:
  - Chuyển đổi bộ nhận diện sang mẫu **3D Isometric Network Cube & Orbit** mang phong cách hạ tầng mạng và điện toán đám mây.
  - Phóng to tỷ lệ khối Cube 3D và vòng quỹ đạo node vệ tinh chiếm **88% diện tích khung squircle**, vươn sát mép viền ngoài ("to sát viền") giúp biểu tượng nổi bật và dễ nhận diện ngay cả ở kích thước nhỏ (16px, 24px, 32px).
  - Tinh chỉnh mặt trái với dấu nhắc lệnh Terminal `>_` to bản, mặt phải với 3 khay máy chủ kèm đèn LED trạng thái (Xanh lá, Cyan, Cam Rust), mặt trên là ma trận mạch mạng.
  - Cập nhật đồng bộ vào resource PE `.exe` và icon cửa sổ Windows/Taskbar.

---

## [v1.3-80] - 2026-11-06
### Improved (Sessions Tree Sidebar Polish)
- **Tối ưu cây thư mục Sessions bên sidebar rộng rãi, thoáng mắt chuẩn MobaXterm**:
  - **Tăng kích thước & chiều cao hàng thư mục**: Nới lỏng khoảng đệm nút thư mục `button_padding = vec2(6.0, 5.5)` và khoảng cách dọc `item_spacing.y = 4.0`, nâng chiều cao mỗi hàng lên ~27-28px cho hit-box thoải mái, dễ bấm.
  - **Tăng cỡ chữ thư mục to rõ nét**: Nâng font chữ gốc `★ User sessions` lên **13.5px**, các thư mục con lên **13.0px** đậm nét, tăng khoảng cách giữa icon `📁` và tên thư mục.
  - **Loại bỏ vạch kẻ dọc phân cấp rườm rà (`indent_has_left_vline = false`)**: Tắt các đường kẻ nối dọc của egui, sử dụng khoảng thụt lề tự nhiên `indent = 16.0` tạo cảm giác thanh thoát, rộng rãi tương tự MobaXterm.
  - **Mở rộng kích thước thẻ session con**: Tăng chiều cao thẻ session 2 dòng từ 36px lên 38px, font tiêu đề session lên 12.5px cân đối với kích thước hàng thư mục.

---

## [v1.3-79] - 2026-11-06
### Added (Native Windows Application Icon & Resource Embedding)
- **Tích hợp Icon ứng dụng riêng biệt, xóa bỏ hoàn toàn chữ "e" mặc định của egui**:
  - **Window Icon (Title Bar, Taskbar, Alt-Tab)**: Nhúng dữ liệu RGBA độ phân giải cao vào cửa sổ eframe qua `ViewportBuilder::with_icon`, hiển thị sắc nét biểu tượng mạng lưới riêng trên thanh tiêu đề và khi chuyển đổi ứng dụng.
  - **Windows Executable Resource (.exe File Icon)**: Tạo file `.ico` đa độ phân giải (16px, 24px, 32px, 48px, 64px, 128px, 256px) và tích hợp `build.rs` biên dịch file tài nguyên `rustybox.rc` nhúng trực tiếp vào section `.rsrc` của PE binary thông qua MinGW `windres`. File `.exe` hiển thị icon riêng trực tiếp trên Windows Explorer và Desktop.

---

## [v1.3-78] - 2026-11-06
### Added & Improved (SFTP Remote Sidebar Parity with MobaXterm)
- **Nâng cấp toàn diện SFTP Sidebar đạt độ tương đồng 100% với MobaXterm gốc**:
  - **Đầy đủ 6 cột dữ liệu**: Bổ sung hiển thị trọn vẹn **Name**, **Size (KB)**, **Last modified**, **Owner**, **Group**, và **Access** (UNIX permissions `drwxrwxr-x`, `-rw-r--r--`).
  - **Tự động ánh xạ UID/GID sang Username/Groupname thật**: SFTP backend worker tự động đọc và cache bảng `/etc/passwd` và `/etc/group` qua SFTP stream khi kết nối, hiển thị tên tài khoản thật (`php`, `root`) thay vì ID số.
  - **Sắp xếp linh hoạt đa cột (Multi-column sorting)**: Hỗ trợ click header để sort theo bất kỳ cột nào kèm mũi tên chỉ hướng tăng/giảm dần (`▲`/`▼`).
  - **Hỗ trợ thanh cuộn ngang (Horizontal Scrolling)**: Cho phép kéo ngang xem trọn vẹn các cột Owner/Group/Permissions ngay cả khi thu hẹp bề rộng sidebar.

---

## [v1.3-77] - 2026-11-06
### Improved (Home Dashboard Balance)
- **Mở rộng giới hạn Recent Connections từ 5 lên 6 kết nối**:
  - Tăng giới hạn `recent_sessions.take(5)` lên `take(6)` trong `src/app.rs`.
  - Kết hợp với cấu trúc chia lưới 3 cột (`chunks(3)`), tạo thành lưới hoàn chỉnh **2 hàng x 3 cột** cân xứng tuyệt đối với 6 thẻ công cụ Action Cards phía trên, loại bỏ hoàn toàn góc trống bất đối xứng.

---

## [v1.3-76] - 2026-11-06
### Fixed & Improved (UI Interaction & Layout Polish)
- **Khắc phục triệt để lỗi chồng lấn/cấn viền khi rê chuột giữa các thẻ**:
  - Phát hiện và xử lý nguyên nhân gốc: `ui.allocate_new_ui(...)` khi gọi trên `ui` cha trong `egui` tự động cập nhật lại con trỏ layout (cursor) dựa theo bounding box của nội dung bên trong, kéo giật lùi vị trí thẻ kế tiếp và gây chồng lấn hitbox/viền sáng lên thẻ trước đó.
  - Chuyển toàn bộ cơ chế dựng nội dung bên trong thẻ của Action Cards và Recent Connections sang `ui.new_child(...)` độc lập, bảo toàn tuyệt đối con trỏ layout của `ui` cha.
  - Chuẩn hóa khoảng cách ngang `card_spacing = 14px` cố định cho từng hàng Action Cards và Recent Connections, đảm bảo các thẻ cách đều nhau 14px, viền hover màu xanh sáng trọn vẹn 4 góc bo 8px, không còn bất kỳ hiện tượng đè mép hay cấn góc nào khi rê chuột.

---

## [v1.3-75] - 2026-11-06
### Fixed & Improved (Home Dashboard Balance)
- **Cân đối khoảng đệm trên/dưới và kích thước thẻ Action Cards**:
  - Tách riêng thông số chiều cao `action_card_h = 58px` (cho 6 thẻ công cụ chính) và `session_card_h = 68px` (cho thẻ Recent Connections 3 dòng).
  - Tinh chỉnh khoảng đệm đối xứng `10px` trên và `10px` dưới cho Action Cards, loại bỏ hoàn toàn khoảng trống dư thừa 22px dưới đáy thẻ.
  - Tăng khoảng cách dọc giữa Hàng 1 và Hàng 2 của Action Cards lên 14px, tạo độ giãn cách thoáng đãng và hài hòa với lưới giao diện tổng thể.

---

## [v1.3-74] - 2026-11-05
### Added & Fixed (Merge PR #4 from david-roark)
- **Tùy chọn Font chữ Terminal & Nhúng JetBrains Nerd Mono**:
  - Tích hợp thêm font lập trình **JetBrains Nerd Mono** hỗ trợ đầy đủ các biểu tượng lập trình (Dev icons, Git glyphs).
  - Bổ sung tùy chọn chọn font chữ terminal trong **Settings**, menu bar (**🔤 Terminal Font**) và hộp thoại cấu hình Session.
  - Hỗ trợ các bộ font monospace: *Noto Sans Mono* (mặc định phủ đầy đủ tiếng Việt), *JetBrains Nerd Mono*, *Ubuntu Mono*, *DejaVu Sans Mono*.
- **Đồng bộ Theme Terminal tức thời & Bảng màu ANSI 16 chuẩn**:
  - Khắc phục hiện tượng chọn theme (Dracula, Nord, Monokai...) không áp dụng vào các session terminal đang mở.
  - Cài đặt đầy đủ 16 mã màu chuẩn ANSI tương ứng với từng theme giao diện (Dracula, Nord, Solarized Dark/Light, Monokai, Matrix, RustyBox Dark).
  - Tự động lưu cấu hình font chữ và theme vào preferences, yêu cầu repaint khung hình ngay lập tức khi người dùng thay đổi.

---

## [v1.3-73] - 2026-11-05
### Fixed & Improved (UI Polish)
- **Nới rộng kích thước thẻ Recent Connections & Home Cards**:
  - Tăng chiều cao thẻ kết nối gần đây từ `56px` lên `70px`, nới lỏng giới hạn clip rect; loại bỏ hoàn toàn hiện tượng dòng thứ 3 (thời gian và số lần kết nối `🕒 YYYY-MM-DD HH:mm (Nx)`) bị cấn đáy và cắt ngang nửa dưới chữ số.
  - Mở rộng chiều rộng thẻ từ `236px` lên `270px` (tổng bề rộng lưới 3 cột đạt `834px`), tăng thêm 34px không gian bề ngang cho mỗi thẻ, giúp tên máy chủ, tài khoản và nhãn thư mục phân loại (`[folder]`) hiển thị trọn vẹn, không bị tràn hay hiển thị dấu ba chấm (`...`).

---

## [v1.3-72] - 2026-11-05
### Fixed & Improved (Windows Experience & UI Polish)
- **Ẩn hoàn toàn cửa sổ Console phụ trên Windows**:
  - Khai báo chỉ thị `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` trong `src/main.rs`.
  - Chuyển đổi PE Subsystem sang `Windows GUI` thuần túy, loại bỏ hoàn toàn cửa sổ dòng lệnh đen chạy ngầm khi mở file `.exe` từ Windows Explorer.
- **Tự động mở Maximize khi khởi động**:
  - Gỡ bỏ giới hạn kích thước cố định `with_inner_size` gây ghi đè cờ phóng to cửa sổ của hệ điều hành.
  - Bổ sung lệnh gửi viewport command `ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true))` ngay ở khung hình đầu tiên, đảm bảo ứng dụng luôn hiển thị toàn màn hình tối ưu không gian làm việc.
- **Căn chỉnh hoàn hảo khối Action Cards trên Home Dashboard**:
  - Điều chỉnh layout con của 6 thẻ công cụ chính từ `Align::Center` sang `Align::Min`, căn thẳng hàng ngang chính xác giữa khung icon badge và dòng tiêu đề.
  - Chuẩn hóa chiều cao card `card_h` về 68px với khoảng đệm trên/dưới cân đối (10px).
  - Cố định `item_spacing.x = card_spacing` cho cả 2 hàng Action Cards và khối Recent Connections, loại bỏ hoàn toàn hiện tượng lệch bước ngang.

---

## [v1.3-68] - 2026-11-05
### Added & Improved (PR #3 & UI Polish)
- **Merge Pull Request #3 from Dtech (SFTP Pool & Transfer Resilience)**:
  - **Multi-Server SFTP Session Pool**: Quản lý pool SFTP độc lập cho từng server từ xa, cho phép chuyển tab terminal mượt mà không block luồng UI và duy trì ngữ cảnh duyệt file riêng cho từng phiên.
  - **Transfer Progress & Cancellation**: Theo dõi tiến độ truyền tệp thời gian thực (bytes transferred, speed, ETA) với khả năng hủy bỏ tiến trình truyền tệp bất cứ lúc nào.
  - **Conflict Resolution & Pre-flight Disk Check**: Tự động phát hiện xung đột tệp tin (Overwrite, Skip, Auto-Rename) và kiểm tra dung lượng ổ đĩa khả dụng trước khi tải xuống nhằm ngăn chặn lỗi tràn đĩa.
  - **Dynamic Sidebar Resizing**: Cho phép kéo giãn thanh bên trái linh hoạt lên tới 50% chiều rộng màn hình kèm thanh chỉ báo trực quan (splitter handle).
  - **Remote Editor & Streaming I/O**: Nâng cấp trình biên tập tệp từ xa, giới hạn kích thước tệp an toàn, cải tiến bộ lọc/sắp xếp danh sách và đồng bộ thư mục làm việc theo terminal prompt.
- **Tối ưu hóa UI/UX Màn hình Home & Form Dialogs**:
  - **Căn giữa toàn diện màn hình Home**: Căn giữa đối xứng cho Hero Quick Connect, lưới 6 thẻ công cụ (Local Terminal, SSH, SFTP, NetScan, Tunnels, Daemons), thanh Filter và danh sách Recent Connections.
  - **Cây thư mục Sessions thông minh**: `★ User sessions` mở sẵn mặc định, các thư mục con thu gọn mặc định (`default_open(false)`) chống tràn màn hình; tự động mở rộng khi gõ tìm kiếm.
  - **Nâng cấp độ tương phản và loại bỏ màn tối scrim**: Cải tiến bộ `Visuals` với màu nền nổi khối `(26, 32, 46)`, viền xanh sáng rõ nét và đổ bóng mềm; loại bỏ lớp scrim đen làm tối các cửa sổ modal.
  - **Mở rộng kích thước form**: Nới rộng New Session Dialog lên `880x580`, tách trường nhập SSH thành 3 hàng thoáng đãng, nâng cấp checkbox 16px sắc nét; mở rộng các modal Snippets, Tunnels, Settings, Passwords.
  - **Phím tắt chuyển Tab linh hoạt**: Hỗ trợ đầy đủ `Ctrl+Tab` / `Ctrl+PageDown` (Next Tab) và `Ctrl+Shift+Tab` / `Ctrl+PageUp` (Prev Tab).
  - **Tinh gọn Sidebar & Home**: Bỏ nút New/Import trùng lặp ở sidebar; bỏ mục Saved Sessions ở Home và giới hạn 5 kết nối gần nhất.

---

## [v1.3-61] - 2026-11-05
### Added
- **Command Snippets Library (Termius Inspired)**:
  - Quản lý kho lệnh DevOps/SysAdmin chuyên sâu (Docker, Nginx, System, Database, Git) trong module `src/snippets/`.
  - Hỗ trợ tham số biến động `{{variable_name}}`: Tự động nhận diện và hiển thị popup điền giá trị tham số trực quan trước khi thực thi.
  - Tích hợp nút `▶ Run` (chạy ngay kèm `\n`) và `📋 Insert` (chèn vào dấu nhắc lệnh) trực tiếp vào Terminal tab đang kích hoạt.
  - Modal quản lý Snippets hoàn chỉnh: Bộ lọc nhóm, tìm kiếm theo từ khóa/lệnh/tag, thêm/sửa/xóa và lưu trữ an toàn trong cấu hình.
- **Visual SSH Tunnels & Port Forwarding Dashboard**:
  - Quản lý đa luồng Tokio async forwarding cho 3 chế độ đường hầm trong `src/tunneling/`:
    - Local Forwarding (`-L`): Lắng nghe port nội bộ chuyển tiếp tới cơ sở dữ liệu / dịch vụ nội bộ remote qua SSH.
    - Remote Forwarding (`-R`): Mở port trên server remote chuyển tiếp ngược về máy phát triển local.
    - Dynamic Forwarding (`-D`): SOCKS5 Proxy Server nội bộ duyệt web và định tuyến qua IP của VPS.
  - Giao diện sơ đồ mạng trực quan (`Local ──SSH Gateway──➔ Remote`), badge Active/Inactive, nút Bật/Tắt 1-click độc lập.
- **Real-Time Auto-Highlight Keywords (ZOC Terminal Inspired)**:
  - Tự động tô màu nổi bật các từ khóa log quan trọng (`ERROR`, `FAIL`, `WARN`, `SUCCESS`, `PANIC`, địa chỉ IPv4) trực tiếp tại tầng render PTY Screen/VT100 của `TerminalWidget`.
  - Hoạt động thuần túy phía client, không can thiệp hay sửa đổi bất kỳ tệp cấu hình/shell nào trên máy chủ từ xa.
  - Thuật toán quét hiệu năng cao 1 lần/dòng theo chỉ mục UTF-8, hỗ trợ kiểm tra ranh giới từ (`whole_word`) và phân biệt hoa/thường (`match_case`).
  - Hộp thoại quản lý danh sách quy tắc với 16 bộ quy tắc mặc định, bảng màu swatch trực quan và khung Live Preview kiểm tra log tức thời.
- **SFTP Explorer Context Menu Đạt 100% Tính Năng Chuẩn MobaXterm**:
  - Context menu chuột phải chuyên sâu cho File (12 tác vụ: Open, Edit with, Diff Viewer, Download, Duplicate, Rename, Chmod permissions, Copy Path/Name, Delete), Folder và Vùng trống.
  - Tích hợp Diff Viewer so sánh nội dung 2 tập tin.
  - Nhấp chuột giữa (Middle-click) vào tệp/thư mục để dán nhanh đường dẫn vào cửa sổ dòng lệnh.

### Fixed & Improved (QA Audit)
- **Modal Dimming Backdrop Scrim**:
  - Bổ sung lớp phủ đen mờ (`Color32::from_black_alpha(150)`) cho toàn bộ modal dialogs (Snippets, Tunnels, Highlight, Passwords, Settings, Diff Viewer), loại bỏ hoàn toàn hiện tượng chữ terminal nền vẽ đè lên viền modal.
- **Auto-Highlight Modal Layout & Multi-line Log Fitting**:
  - Sửa lỗi tràn chiều ngang và lỗi glyph ký tự ngắt dòng trong modal Auto-Highlight; cố định dải kích thước cửa sổ và tối ưu form nhập 2 hàng cân đối.
- **Session Cards Clip Rect Boundary**:
  - Khắc phục triệt để hiện tượng thẻ session cuộn đè lấn lên thanh trạng thái Bottom Bar bằng cơ chế giao cắt (`intersect`) clip rect.
- **Duy trì 94/94 Unit & Stress Tests Pass 100%**:
  - Đo kiểm độ ổn định bộ nhớ đạt chuẩn (dao động 20 KB sau 50 chu kỳ tương tác GUI liên tục).

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
