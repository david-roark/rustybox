# 🚀 RustyBox - Cross-Platform Remote Terminal Toolbox (Pure Rust)

> **Bộ công cụ terminal & remote toolbox đa nền tảng:** Viết 100% bằng **Rust**, mã nguồn mở, miễn phí, chạy ngay không cần cài đặt, hỗ trợ **Windows, macOS và Linux**.

---

## 📊 Thông số chính

* **Nền tảng:** Windows, macOS, Ubuntu/Linux
* **Công nghệ lõi:** 100% Native Rust (`egui` GPU render + `portable-pty` + `vt100`)
* **Chế độ lưu cấu hình:** tự lưu `data/RustyBox.json` cạnh file thực thi (mật khẩu mã hóa Argon2id + AES-256-GCM)


---

## ✨ Các tính năng đã được triển khai hoàn chỉnh

### 1. Tabbed Terminal & Multi-Split View
* **Đa tab mượt mà:** Mở không giới hạn các tab Local Shell và SSH.
* **Chế độ chia màn hình (Split View):**
  * `Single`: Chế độ 1 màn hình chuẩn.
  * `2-Cols (Horizontal)`: Chia đôi trái/phải xem đồng thời 2 server.
  * `2-Rows (Vertical)`: Chia đôi trên/dưới.
  * `2x2 Grid`: Chia 4 góc xem đồng thời 4 máy chủ cùng lúc.
* **Bộ giả lập VT100 / Xterm:** Hỗ trợ đầy đủ bảng màu 256 ANSI, 24-bit TrueColor, con trỏ nhấp nháy, phím điều hướng và phím tắt chuẩn terminal (`Ctrl+C`, `Ctrl+D`, `Ctrl+Z`, `Ctrl+L`...).

### 2. Multi-Execution (Phát lệnh đồng thời)
* Tính năng độc quyền nổi tiếng của RustyBox: Nút gạt **`⚡ Multi-Exec`** trên thanh công cụ.
* Khi bật, mọi ký tự hoặc câu lệnh bạn gõ trên tab hiện tại sẽ được **tự động phát thanh (broadcast) đến tất cả các tab server đang mở cùng lúc**! Giúp quản trị viên cập nhật hoặc kiểm tra cấu hình hàng loạt máy chủ chỉ trong 1 thao tác.

### 3. Graphical SFTP Browser (Sidebar quản lý tệp đồ họa)
* **Tự động bám theo Session:** Khi mở kết nối SSH, sidebar bên trái chuyển sang trình duyệt tệp đồ họa.
* **Thao tác tệp trực quan:**
  * Xem cây thư mục, kích thước tệp định dạng đẹp (KB, MB, GB).
  * Nút `⬆ Up` lên thư mục cha, thanh Breadcrumb nhảy nhanh tới đường dẫn bất kỳ.
  * `📤 Upload`: Chọn tệp từ máy tính tải lên server (hỗ trợ hộp thoại file native).
  * `📥 Download`: Tải tệp từ server về máy tính cá nhân.
  * `🗑 Delete`: Xóa tệp/thư mục.
* **📝 Quick Editor tích hợp:** Click `Edit` để mở ngay trình soạn thảo code/text tích hợp sẵn bên trong app để xem hoặc sửa file cấu hình từ xa và bấm `Save` lưu ngay lập tức!

### 4. Quản lý danh bạ máy chủ (Session Tree)
* Phân chia nhóm / thư mục theo nhu cầu (`Production`, `Staging`, `Database`, `Default`).
* Double-click để kết nối tức thì.
* Menu chuột phải: Kết nối, Sửa, Nhân bản, Xóa.
* Hỗ trợ tìm kiếm lọc nhanh (Filter).
* **Xuất / Nhập danh bạ:** Export và Import định dạng JSON tiêu chuẩn.

### 5. SSH Port Forwarding / Tunnels
* Quản lý trực quan các đường hầm mạng:
  * **Local Port Forwarding:** Chuyển tiếp cổng máy local sang server nội bộ (ví dụ: truy cập MySQL `3306` của server từ cổng `3307` máy cá nhân qua JumpHost).
  * **Dynamic Forwarding:** Thiết lập SOCKS5 Proxy.
* Đèn trạng thái `🟢 Running` / `⚪ Stopped` và nút bật/tắt tức thì.

### 6. Công cụ mạng tích hợp (Network Tools)
* **Port Scanner:** Quét siêu nhanh các cổng phổ biến (SSH, HTTP, HTTPS, MySQL, PostgreSQL, RDP, VNC, Redis...) bằng Tokio async kèm đo độ trễ (latency ms).
* **TCP Ping Utility:** Kiểm tra độ trễ mạng liên tục theo thời gian thực.

### 7. Bảo mật & Master Password Vault
* Mã hóa toàn bộ thông tin đăng nhập và danh bạ máy chủ bằng thuật toán **AES-256-GCM** với khóa dẫn xuất **SHA-256**.
* Khóa an toàn toàn bộ ứng dụng bằng một Master Password.

---

## 🛠 Hướng dẫn biên dịch và sử dụng

### 1. Chạy trực tiếp từ mã nguồn:
```bash
git clone https://github.com/longkaka/rust-remote-toolbox.git
cd rust-remote-toolbox
cargo run --release
```

### 2. File thực thi:
Sau khi biên dịch release:
* File thực thi tạo ra tại `target/release/rust-remote-toolbox` (trên Windows là `rust-remote-toolbox.exe`).
* Dung lượng chỉ **~8.4 MB**.
* Để dùng ở chế độ Portable (USB mang đi mọi nơi): Chỉ cần đặt file `data/RustyBox.json` bên cạnh file `.exe`, ứng dụng sẽ tự động lưu dữ liệu vào đó mà không ghi bất kỳ file rác nào vào hệ thống!

---

## 🌐 Tự động Build đa nền tảng (GitHub Actions)
Dự án đã cấu hình sẵn quy trình CI/CD `.github/workflows/build-release.yml` tự động build ra 4 file binary:
1. `RustyBox-windows-x64.exe` (Windows 10/11)
2. `RustyBox-macos-arm64` (macOS Apple Silicon M1/M2/M3/M4)
3. `RustyBox-macos-x64` (macOS Intel)
4. `RustyBox-linux-x64` (Ubuntu / Debian / Linux)
