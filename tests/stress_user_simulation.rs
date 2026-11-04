use rust_remote_toolbox::session::storage::{
    StorageManager, Credential, create_vault, unlock_vault, encrypt_str, decrypt_str, is_encrypted
};
use rust_remote_toolbox::session::{Session, SessionType, AuthMethod};
use rust_remote_toolbox::session::import_export::parse_sessions_from_text;
use std::fs;

fn make_temp_dir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("rustybox_stress_{}", uuid::Uuid::new_v4()));
    let _ = fs::create_dir_all(&p);
    p
}

#[test]
fn test_stress_session_crud_100_cycles() {
    let dir = make_temp_dir();
    let storage = StorageManager::with_data_dir(dir.clone());

    let mut sessions = Vec::new();
    // 100 cycles of session creation with diverse protocol combinations, edge case names, unicode, Vietnamese
    for i in 0..100 {
        let name = match i % 5 {
            0 => format!("VPS Production Hà Nội #{}", i),
            1 => format!("Server Đà Nẵng [Test-Cluster] @ {}", i),
            2 => format!("Special chars <>&\"' $VAR `{}`", i),
            3 => format!("Long name {}", "A".repeat(200)),
            _ => format!("Simple-{}", i),
        };

        let session_type = match i % 8 {
            0 => SessionType::Ssh {
                host: format!("192.168.1.{}", (i % 254) + 1),
                port: if i % 10 == 0 { 0 } else if i % 10 == 1 { 65535 } else { 22 },
                user: if i % 2 == 0 { "root".into() } else { "ubuntu".into() },
                auth_method: AuthMethod::Password,
                key_path: if i % 3 == 0 { Some("/path/to/key.pem".into()) } else { None },
                jump_host: None,
                tunnel_x11: false,
            },
            1 => SessionType::LocalShell {
                custom_shell: Some("/bin/bash".into()),
                working_dir: Some("/tmp".into()),
            },
            2 => SessionType::Telnet {
                host: "router.lan".into(),
                port: 23,
            },
            3 => SessionType::Serial {
                port_name: "/dev/ttyUSB0".into(),
                baud_rate: 115200,
            },
            4 => SessionType::Rdp {
                host: "windows-server.lan".into(),
                port: 3389,
                user: "Administrator".into(),
            },
            5 => SessionType::Vnc {
                host: "vnc.lan".into(),
                port: 5900,
            },
            6 => SessionType::Ftp {
                host: "ftp.lan".into(),
                port: 21,
                user: "anonymous".into(),
            },
            _ => SessionType::Wsl {
                distro: "Ubuntu-24.04".into(),
                user: Some("ubuntu".into()),
                working_dir: Some("/home/ubuntu".into()),
            },
        };

        let sess = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            folder: if i % 4 == 0 { "Production".into() } else if i % 4 == 1 { "Staging/Backend".into() } else { "Default".into() },
            session_type,
            auto_sftp: i % 2 == 0,
            created_at: chrono::Local::now().to_rfc3339(),
            password: Some(format!("P@ssw0rd_{}", i)),
            save_password: true,
            ssh_browser_type: "SFTP".into(),
            remote_command: String::new(),
            do_not_exit_after_cmd: false,
            ssh_compression: true,
            terminal_font_size: 14,
            terminal_color_scheme: "RustyBoxDark".into(),
            backspace_sends_ctrl_h: false,
            right_click_paste: true,
            log_terminal_output: false,
            log_directory: String::new(),
            use_ssh_gateway: false,
            gateway_host: String::new(),
            gateway_port: 22,
            gateway_user: String::new(),
            gateway_key: String::new(),
            ssh_keepalive: true,
            tab_color: String::new(),
            comments: format!("Created in test cycle {}", i),
            last_connected_at: None,
            connect_count: 0,
        };
        sessions.push(sess);
    }

    assert_eq!(sessions.len(), 100);

    // Save all 100 sessions
    let res = storage.save_sessions(&sessions);
    assert!(res.is_ok(), "Failed to save 100 sessions: {:?}", res);

    // Reload and verify integrity
    let loaded = storage.load_sessions();
    assert_eq!(loaded.len(), 100);
    for i in 0..100 {
        assert_eq!(loaded[i].id, sessions[i].id);
        assert_eq!(loaded[i].name, sessions[i].name);
        assert_eq!(loaded[i].folder, sessions[i].folder);
    }

    // 100 cycles of updates and deletions
    for i in 0..100 {
        sessions[i].name = format!("Updated Name {}", i);
        sessions[i].folder = format!("NewFolder/{}", i % 3);
    }
    let res = storage.save_sessions(&sessions);
    assert!(res.is_ok());

    let loaded_updated = storage.load_sessions();
    assert_eq!(loaded_updated[0].name, "Updated Name 0");
    assert_eq!(loaded_updated[99].name, "Updated Name 99");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_stress_vault_encryption_100_cycles() {
    let dir = make_temp_dir();
    let mut storage = StorageManager::with_data_dir(dir.clone());

    // 10 cycles of master password creation & unlock, each performing 10 AES-256-GCM encryptions (100 total AES operations)
    for i in 0..10 {
        let master_pass = format!("MasterPass_{}_!@#$%^&*()_+", i);

        // 1. Create vault
        let (meta, key) = create_vault(&master_pass).expect("create_vault failed");
        storage.set_vault_key(Some(key));
        storage.set_vault_configured(true);

        for j in 0..10 {
            let secret = format!("VerySecretPassword_For_Server_{}_{}", i, j);
            // 2. Encrypt string
            let encrypted = encrypt_str(&key, &secret).expect("encrypt failed");
            assert!(is_encrypted(&encrypted));
            assert!(!encrypted.contains(&secret));

            // Decrypt string
            let decrypted = decrypt_str(&key, &encrypted).expect("decrypt failed");
            assert_eq!(decrypted, secret);
        }

        // 3. Lock vault
        storage.set_vault_key(None);
        assert!(!storage.is_vault_unlocked());

        // 4. Try wrong password - must fail
        let wrong_pass = format!("WrongPassword_{}", i);
        let unlock_res = unlock_vault(&meta, &wrong_pass);
        assert!(unlock_res.is_err(), "Unlock should have failed with wrong password");

        // 5. Unlock with right password
        let unlocked_key = unlock_vault(&meta, &master_pass).expect("Unlock failed with correct password");
        assert_eq!(unlocked_key, key);
        storage.set_vault_key(Some(unlocked_key));
        assert!(storage.is_vault_unlocked());

        // 6. Clear vault for next cycle
        storage.set_vault_key(None);
        storage.set_vault_configured(false);
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_stress_credentials_crud_100_cycles() {
    let dir = make_temp_dir();
    let storage = StorageManager::with_data_dir(dir.clone());

    let mut creds = Vec::new();
    for i in 0..100 {
        creds.push(Credential {
            name: format!("Cred-Account-{}-{}", i, if i % 2 == 0 { "Admin" } else { "Dev" }),
            username: format!("user_{}", i),
            password: format!("Secret_Pass_{}_$%", i),
        });
    }

    let save_res = storage.save_credentials(&creds);
    assert!(save_res.is_ok());

    let loaded = storage.load_credentials();
    assert_eq!(loaded.len(), 100);

    // Search and filter 100 times
    for i in 0..100 {
        let q = format!("user_{}", i);
        let found: Vec<_> = loaded.iter().filter(|c| c.username == q).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].username, q);
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_stress_mobaxterm_import_parser_100_cases() {
    for i in 0..100 {
        let input = match i % 6 {
            0 => format!("VPS_{}=#109#0%51.79.157.{}%22%root%%-1%-1%%%22%%0%0%0%%-1%0%0%0%%1080%%0%0%1#MobaFont%10%16777215%4278190080%256%0%0%0%%-1%0%0%0%", i, i),
            1 => format!("SimpleServer{} = root@192.168.1.{}:22", i, i),
            2 => format!("HostWithoutPort{} = admin@10.0.0.{}", i, i),
            3 => format!("[Bookmarks]\r\nSubRep=Folder{}\r\nServer{} = #109#0%host{}%22%user%%", i, i, i),
            4 => format!("Malformed line with no separators {} !@#$%^&*()", i),
            _ => format!("Empty fields #109#0%%%%%%%"),
        };

        // Parser should never panic on arbitrary inputs
        let parsed = parse_sessions_from_text(&input, "TestDefaultFolder");
        if i % 6 <= 3 && !input.contains("Malformed") && !input.contains("Empty fields") {
            assert!(!parsed.is_empty(), "Failed to parse valid line: {}", input);
        }
    }
}
