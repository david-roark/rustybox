use super::{Session, SessionType};
use std::collections::HashSet;

/// Parsed session result
#[derive(Clone, Debug)]
pub struct ImportSummary {
    pub total_parsed: usize,
    pub new_imported: usize,
    pub duplicates_skipped: usize,
}

/// Parses session lines from MobaXterm TXT format, INI/mxtpro format, JSON, or CSV
pub fn parse_sessions_from_text(content: &str, default_folder: &str) -> Vec<Session> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // 1. Try parsing as JSON first
    if let Ok(sessions) = serde_json::from_str::<Vec<Session>>(trimmed) {
        return sessions;
    }
    if let Ok(cfg) = serde_json::from_str::<crate::session::storage::RustyBoxConfig>(trimmed) {
        return cfg.sessions;
    }

    let mut sessions = Vec::new();
    let mut current_folder = if default_folder.trim().is_empty() {
        "Imported".to_string()
    } else {
        default_folder.trim().to_string()
    };

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // MobaXterm INI section or SubRep
        if line.starts_with('[') && line.ends_with(']') {
            continue;
        }
        if line.starts_with("SubRep=") {
            let folder_part = &line["SubRep=".len()..];
            if !folder_part.trim().is_empty() {
                current_folder = folder_part.trim().to_string();
            }
            continue;
        }

        // 2. MobaXterm INI bookmark line:
        // Name=#109#1#Host#Port#User#... or Name = #109#0%Host%Port%User%...
        if let Some((left, right)) = line.split_once('=') {
            let right_trimmed = right.trim();
            if right_trimmed.starts_with('#') {
                let name = left.trim();
                let def_part = right_trimmed.trim_start_matches('#');
                // Tokens can be separated by '#' or '%'
                let tokens: Vec<&str> = def_part.split(|c| c == '#' || c == '%').collect();
                // Typically: type_id (e.g. 109=SSH, 91=RDP), opt/sub_type, host, port, user...
                if tokens.len() >= 5 {
                    let type_id = tokens[0].trim();
                    let host = tokens[2].trim();
                    let port: u16 = tokens[3].trim().parse().unwrap_or(22);
                    let user = tokens[4].trim();

                    if !host.is_empty() {
                        let mut s = if type_id == "109" || type_id == "1" {
                            Session::new_ssh(
                                if name.is_empty() { host } else { name },
                                &current_folder,
                                host,
                                port,
                                if user.is_empty() { "root" } else { user },
                            )
                        } else if type_id == "91" {
                            Session {
                                id: uuid::Uuid::new_v4().to_string(),
                                name: if name.is_empty() { host.to_string() } else { name.to_string() },
                                folder: current_folder.clone(),
                                session_type: SessionType::Rdp {
                                    host: host.to_string(),
                                    port,
                                    user: user.to_string(),
                                },
                                auto_sftp: false,
                                created_at: chrono::Local::now().to_rfc3339(),
                                password: None,
                                save_password: false,
                                ..Session::new_local(name, &current_folder)
                            }
                        } else {
                            Session::new_ssh(
                                if name.is_empty() { host } else { name },
                                &current_folder,
                                host,
                                port,
                                if user.is_empty() { "root" } else { user },
                            )
                        };
                        s.save_password = true;
                        sessions.push(s);
                        continue;
                    }
                }
            }
        }

        // 3. Format: [proto:][user]@[host][:port] = [password]
        // Examples:
        // web2@207.148.67.131 = VOUmxTQK3q
        // ssh8922:vas_admin@10.230.194.33 = iFY9#SRuo%5W
        // ssh24700:ruby@103.57.220.111 = yU5o)o7vsJPACZ$=ntbZ
        // vnc:vncuser@45.82.250.75 = SwevW3f0
        if let Some(eq_pos) = line.find('=') {
            let left = line[..eq_pos].trim();
            let password = line[eq_pos + 1..].trim();

            if let Some(at_pos) = left.find('@') {
                let user_part = &left[..at_pos];
                let host_part = &left[at_pos + 1..];

                let mut port: u16 = 22;
                let mut username = user_part;
                let mut is_vnc = false;

                // Check prefix in user_part like "ssh24700:ruby" or "ssh:ruby" or "vnc:user"
                if let Some(colon_pos) = user_part.find(':') {
                    let prefix = &user_part[..colon_pos];
                    username = &user_part[colon_pos + 1..];

                    if prefix.starts_with("ssh") {
                        let num_part = &prefix[3..];
                        if let Ok(p) = num_part.parse::<u16>() {
                            port = p;
                        }
                    } else if prefix.starts_with("vnc") {
                        is_vnc = true;
                        port = 5900;
                    }
                }

                // Check port in host_part like "103.57.220.111:2222"
                let host = if let Some(colon_pos) = host_part.find(':') {
                    if let Ok(p) = host_part[colon_pos + 1..].parse::<u16>() {
                        port = p;
                    }
                    &host_part[..colon_pos]
                } else {
                    host_part
                };

                let host = host.trim();
                let username = username.trim();

                if !host.is_empty() {
                    let session_name = format!("{}:{}", host, port);
                    let s = if is_vnc {
                        Session {
                            id: uuid::Uuid::new_v4().to_string(),
                            name: session_name,
                            folder: current_folder.clone(),
                            session_type: SessionType::Vnc {
                                host: host.to_string(),
                                port,
                            },
                            auto_sftp: false,
                            created_at: chrono::Local::now().to_rfc3339(),
                            password: if password.is_empty() { None } else { Some(password.to_string()) },
                            save_password: !password.is_empty(),
                            ..Session::new_local(&host, &current_folder)
                        }
                    } else {
                        let mut sess = Session::new_ssh(
                            &session_name,
                            &current_folder,
                            host,
                            port,
                            if username.is_empty() { "root" } else { username },
                        );
                        if !password.is_empty() {
                            sess.password = Some(password.to_string());
                            sess.save_password = true;
                        }
                        sess
                    };
                    sessions.push(s);
                    continue;
                }
            }
        }

        // 4. Simple host line: user@host[:port] or host[:port]
        if line.contains('.') || line.contains(':') {
            let (user, host_port) = if let Some(at_pos) = line.find('@') {
                (&line[..at_pos], &line[at_pos + 1..])
            } else {
                ("root", line)
            };

            let (host, port) = if let Some(colon_pos) = host_port.find(':') {
                let p = host_port[colon_pos + 1..].parse::<u16>().unwrap_or(22);
                (&host_port[..colon_pos], p)
            } else {
                (host_port, 22)
            };

            let host = host.trim();
            if !host.is_empty() && host.contains('.') {
                let s = Session::new_ssh(
                    &format!("{}:{}", host, port),
                    &current_folder,
                    host,
                    port,
                    user.trim(),
                );
                sessions.push(s);
            }
        }
    }

    sessions
}

/// Merges imported sessions into target vector, skipping duplicate host:port:user combinations
pub fn merge_imported_sessions(
    existing: &mut Vec<Session>,
    imported: Vec<Session>,
) -> ImportSummary {
    let total_parsed = imported.len();
    let mut duplicates_skipped = 0;
    let mut new_imported = 0;

    let mut existing_keys: HashSet<String> = HashSet::new();
    for s in existing.iter() {
        match &s.session_type {
            SessionType::Ssh { host, port, user, .. } => {
                existing_keys.insert(format!("ssh:{}:{}:{}", host.trim().to_lowercase(), port, user.trim()));
            }
            SessionType::Vnc { host, port } => {
                existing_keys.insert(format!("vnc:{}:{}", host.trim().to_lowercase(), port));
            }
            SessionType::Rdp { host, port, user } => {
                existing_keys.insert(format!("rdp:{}:{}:{}", host.trim().to_lowercase(), port, user.trim()));
            }
            _ => {
                existing_keys.insert(s.name.clone());
            }
        }
    }

    for mut s in imported {
        let key = match &s.session_type {
            SessionType::Ssh { host, port, user, .. } => {
                format!("ssh:{}:{}:{}", host.trim().to_lowercase(), port, user.trim())
            }
            SessionType::Vnc { host, port } => {
                format!("vnc:{}:{}", host.trim().to_lowercase(), port)
            }
            SessionType::Rdp { host, port, user } => {
                format!("rdp:{}:{}:{}", host.trim().to_lowercase(), port, user.trim())
            }
            _ => s.name.clone(),
        };

        if existing_keys.contains(&key) {
            duplicates_skipped += 1;
        } else {
            existing_keys.insert(key);
            // Ensure distinct UUID
            s.id = uuid::Uuid::new_v4().to_string();
            existing.push(s);
            new_imported += 1;
        }
    }

    ImportSummary {
        total_parsed,
        new_imported,
        duplicates_skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_moba_txt_formats() {
        let text = r#"
web2@207.148.67.131 = VOUmxTQK3q
ssh8922:vas_admin@10.230.194.33 = iFY9#SRuo%5W
ssh24700:ruby@103.57.220.111 = yU5o)o7vsJPACZ$=ntbZ
vnc:vncuser@45.82.250.75 = SwevW3f0
"#;
        let parsed = parse_sessions_from_text(text, "Servers");
        assert_eq!(parsed.len(), 4);

        // 1. web2
        assert_eq!(parsed[0].folder, "Servers");
        if let SessionType::Ssh { ref host, port, ref user, .. } = parsed[0].session_type {
            assert_eq!(host, "207.148.67.131");
            assert_eq!(port, 22);
            assert_eq!(user, "web2");
        } else {
            panic!("Expected SSH session");
        }
        assert_eq!(parsed[0].password.as_deref(), Some("VOUmxTQK3q"));

        // 2. ssh8922:vas_admin
        if let SessionType::Ssh { ref host, port, ref user, .. } = parsed[1].session_type {
            assert_eq!(host, "10.230.194.33");
            assert_eq!(port, 8922);
            assert_eq!(user, "vas_admin");
        } else {
            panic!("Expected SSH session with port 8922");
        }

        // 3. ssh24700:ruby
        if let SessionType::Ssh { ref host, port, ref user, .. } = parsed[2].session_type {
            assert_eq!(host, "103.57.220.111");
            assert_eq!(port, 24700);
            assert_eq!(user, "ruby");
        } else {
            panic!("Expected SSH session with port 24700");
        }
        assert_eq!(parsed[2].password.as_deref(), Some("yU5o)o7vsJPACZ$=ntbZ"));

        // 4. vnc
        if let SessionType::Vnc { ref host, port } = parsed[3].session_type {
            assert_eq!(host, "45.82.250.75");
            assert_eq!(port, 5900);
        } else {
            panic!("Expected VNC session");
        }

        // Test merge
        let mut existing = Vec::new();
        let summary = merge_imported_sessions(&mut existing, parsed);
        assert_eq!(summary.total_parsed, 4);
        assert_eq!(summary.new_imported, 4);
        assert_eq!(summary.duplicates_skipped, 0);
        assert_eq!(existing.len(), 4);

        // Second merge should skip duplicates
        let parsed_again = parse_sessions_from_text(text, "Servers");
        let summary2 = merge_imported_sessions(&mut existing, parsed_again);
        assert_eq!(summary2.new_imported, 0);
        assert_eq!(summary2.duplicates_skipped, 4);
        assert_eq!(existing.len(), 4);
    }
}
