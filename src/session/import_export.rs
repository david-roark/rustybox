use super::{AuthMethod, Session, SessionType};
use std::collections::HashSet;
use std::path::PathBuf;

/// Parsed session result
#[derive(Clone, Debug)]
pub struct ImportSummary {
    pub total_parsed: usize,
    pub new_imported: usize,
    pub duplicates_skipped: usize,
}

/// Returns the default OpenSSH client config path (`~/.ssh/config`) if a home directory is available.
pub fn default_ssh_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config"))
}

/// Expands leading `~/` or `~\` to the user's home directory.
fn expand_tilde_path(raw: &str) -> String {
    let clean = raw.trim().trim_matches('"').trim_matches('\'');
    if let Some(rest) = clean.strip_prefix("~/").or_else(|| clean.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    clean.to_string()
}

/// Checks if text contains OpenSSH config `Host <alias>` blocks.
pub fn looks_like_openssh_config(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.contains("=#") {
            continue;
        }
        let mut parts = trimmed.splitn(2, |c: char| c.is_whitespace() || c == '=');
        if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
            if key.eq_ignore_ascii_case("Host") && !val.trim().is_empty() && !val.contains('@') {
                return true;
            }
        }
    }
    false
}

/// Parses standard OpenSSH configuration (`~/.ssh/config`) into RustyBox SSH `Session` profiles.
pub fn parse_openssh_config(content: &str, default_folder: &str) -> Vec<Session> {
    let folder = if default_folder.trim().is_empty() {
        "OpenSSH Config".to_string()
    } else {
        default_folder.trim().to_string()
    };

    #[derive(Default, Clone)]
    struct SshHostBlock {
        aliases: Vec<String>,
        hostname: Option<String>,
        user: Option<String>,
        port: Option<u16>,
        identity_file: Option<String>,
        proxy_jump: Option<String>,
        forward_x11: Option<bool>,
        compression: Option<bool>,
    }

    let mut global_defaults = SshHostBlock::default();
    let mut blocks: Vec<SshHostBlock> = Vec::new();
    let mut current: Option<SshHostBlock> = None;

    let flush_block = |blk: SshHostBlock, blocks: &mut Vec<SshHostBlock>, global: &mut SshHostBlock| {
        if blk.aliases.iter().all(|a| a == "*") {
            if blk.user.is_some() { global.user = blk.user; }
            if blk.port.is_some() { global.port = blk.port; }
            if blk.identity_file.is_some() { global.identity_file = blk.identity_file; }
            if blk.proxy_jump.is_some() { global.proxy_jump = blk.proxy_jump; }
            if blk.forward_x11.is_some() { global.forward_x11 = blk.forward_x11; }
            if blk.compression.is_some() { global.compression = blk.compression; }
        } else {
            blocks.push(blk);
        }
    };

    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, |c: char| c.is_whitespace() || c == '=');
        let Some(key) = parts.next().map(str::trim) else { continue };
        let Some(val) = parts.next().map(|v| v.trim().trim_start_matches('=').trim()) else { continue };
        if val.is_empty() {
            continue;
        }

        if key.eq_ignore_ascii_case("Host") {
            if let Some(prev) = current.take() {
                flush_block(prev, &mut blocks, &mut global_defaults);
            }
            let aliases: Vec<String> = val
                .split_whitespace()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            current = Some(SshHostBlock {
                aliases,
                ..Default::default()
            });
            continue;
        }

        let target = current.get_or_insert_with(|| SshHostBlock {
            aliases: vec!["*".to_string()],
            ..Default::default()
        });

        if key.eq_ignore_ascii_case("HostName") {
            target.hostname = Some(val.trim_matches('"').to_string());
        } else if key.eq_ignore_ascii_case("User") {
            target.user = Some(val.trim_matches('"').to_string());
        } else if key.eq_ignore_ascii_case("Port") {
            if let Ok(p) = val.parse::<u16>() {
                target.port = Some(p);
            }
        } else if key.eq_ignore_ascii_case("IdentityFile") {
            target.identity_file = Some(expand_tilde_path(val));
        } else if key.eq_ignore_ascii_case("ProxyJump") {
            target.proxy_jump = Some(val.trim_matches('"').to_string());
        } else if key.eq_ignore_ascii_case("ForwardX11") || key.eq_ignore_ascii_case("ForwardX11Trusted") {
            target.forward_x11 = Some(val.eq_ignore_ascii_case("yes") || val.eq_ignore_ascii_case("true"));
        } else if key.eq_ignore_ascii_case("Compression") {
            target.compression = Some(val.eq_ignore_ascii_case("yes") || val.eq_ignore_ascii_case("true"));
        }
    }

    if let Some(last) = current.take() {
        flush_block(last, &mut blocks, &mut global_defaults);
    }

    let mut sessions = Vec::new();
    for blk in blocks {
        for alias in &blk.aliases {
            // Skip wildcard patterns like `*`, `*.internal`, `!bastion`
            if alias.contains('*') || alias.contains('?') || alias.starts_with('!') {
                continue;
            }
            let host = blk.hostname.clone().unwrap_or_else(|| alias.clone());
            if host.is_empty() {
                continue;
            }
            let port = blk.port.or(global_defaults.port).unwrap_or(22);
            let user = blk
                .user
                .clone()
                .or_else(|| global_defaults.user.clone())
                .unwrap_or_else(|| "root".to_string());
            let key_path = blk
                .identity_file
                .clone()
                .or_else(|| global_defaults.identity_file.clone());
            let jump_host = blk
                .proxy_jump
                .clone()
                .or_else(|| global_defaults.proxy_jump.clone())
                .filter(|jh| !jh.trim().is_empty() && !jh.trim().eq_ignore_ascii_case("none"));
            let tunnel_x11 = blk
                .forward_x11
                .or(global_defaults.forward_x11)
                .unwrap_or(false);
            let ssh_compression = blk
                .compression
                .or(global_defaults.compression)
                .unwrap_or(true);

            let mut sess = Session::new_ssh(alias, &folder, &host, port, &user);
            sess.ssh_compression = ssh_compression;
            if let Some(ref jh) = jump_host {
                sess.use_ssh_gateway = true;
                let first_hop = jh.split(',').next().unwrap_or(jh).trim();
                let (gw_user, host_port) = if let Some((u, rest)) = first_hop.split_once('@') {
                    (u.trim().to_string(), rest.trim())
                } else {
                    (String::new(), first_hop)
                };
                let (gw_host, gw_port) = if let Some((h, p)) = host_port.rsplit_once(':') {
                    (h.trim().to_string(), p.trim().parse::<u16>().unwrap_or(22))
                } else {
                    (host_port.to_string(), 22)
                };
                sess.gateway_user = gw_user;
                sess.gateway_host = gw_host;
                sess.gateway_port = gw_port;
            }
            sess.session_type = SessionType::Ssh {
                host,
                port,
                user,
                auth_method: if key_path.is_some() {
                    AuthMethod::PrivateKey
                } else {
                    AuthMethod::Password
                },
                key_path,
                jump_host,
                tunnel_x11,
            };
            sessions.push(sess);
        }
    }

    sessions
}

/// Parses session lines from OpenSSH config (`~/.ssh/config`), MobaXterm TXT/INI/mxtpro format, JSON, or CSV
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

    // 1b. Try parsing as OpenSSH config (`~/.ssh/config`) if `Host <alias>` blocks are present
    if looks_like_openssh_config(trimmed) {
        let ssh_cfg_sessions = parse_openssh_config(trimmed, default_folder);
        if !ssh_cfg_sessions.is_empty() {
            return ssh_cfg_sessions;
        }
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

    #[test]
    fn test_parse_openssh_config() {
        let ssh_cfg = r#"
# Global defaults
Host *
    User ubuntu
    ForwardX11 yes

Host prod-api
    HostName 203.0.113.10
    Port 2222
    User deploy
    IdentityFile ~/.ssh/id_ed25519
    ProxyJump bastion.example.com:22

Host db-primary db-replica
    HostName 10.10.0.50
    Port 2200

Host *.internal
    User ignored
"#;
        let parsed = parse_sessions_from_text(ssh_cfg, "OpenSSH");
        assert_eq!(parsed.len(), 3);

        assert_eq!(parsed[0].name, "prod-api");
        assert_eq!(parsed[0].folder, "OpenSSH");
        if let SessionType::Ssh {
            ref host,
            port,
            ref user,
            ref auth_method,
            ref key_path,
            ref jump_host,
            tunnel_x11,
        } = parsed[0].session_type
        {
            assert_eq!(host, "203.0.113.10");
            assert_eq!(port, 2222);
            assert_eq!(user, "deploy");
            assert_eq!(*auth_method, AuthMethod::PrivateKey);
            assert!(key_path.as_deref().unwrap_or("").ends_with(".ssh/id_ed25519"));
            assert_eq!(jump_host.as_deref(), Some("bastion.example.com:22"));
            assert!(tunnel_x11);
        } else {
            panic!("Expected SSH session for prod-api");
        }

        // db-primary inherits User ubuntu from Host *
        assert_eq!(parsed[1].name, "db-primary");
        if let SessionType::Ssh { ref host, port, ref user, .. } = parsed[1].session_type {
            assert_eq!(host, "10.10.0.50");
            assert_eq!(port, 2200);
            assert_eq!(user, "ubuntu");
        } else {
            panic!("Expected SSH session for db-primary");
        }
    }

    #[test]
    fn test_parse_openssh_config_equals_quotes_comments_and_proxyjump() {
        let ssh_cfg = r#"
Host = web-edge # Primary edge node
    HostName = "198.51.100.88" # inline IP comment
    User = "devops"
    Port = 22022
    IdentityFile = "~/.ssh/my edge key.pem"
    ProxyJump = "bastion_user@198.51.100.1:2222"
    ForwardX11Trusted = yes
    Compression = no

Host direct-node *.corp.local !excluded
    HostName 192.0.2.55
    ProxyJump none

# Fallback wildcard at bottom of file
Host *
    User fallback_admin
    Port 2200
    ProxyJump jump.corp.local:22
"#;
        let parsed = parse_sessions_from_text(ssh_cfg, "");
        assert_eq!(parsed.len(), 2);

        // 1. web-edge
        assert_eq!(parsed[0].name, "web-edge");
        assert_eq!(parsed[0].folder, "OpenSSH Config");
        assert!(!parsed[0].ssh_compression);
        assert!(parsed[0].use_ssh_gateway);
        assert_eq!(parsed[0].gateway_user, "bastion_user");
        assert_eq!(parsed[0].gateway_host, "198.51.100.1");
        assert_eq!(parsed[0].gateway_port, 2222);
        if let SessionType::Ssh {
            ref host,
            port,
            ref user,
            ref auth_method,
            ref key_path,
            ref jump_host,
            tunnel_x11,
        } = parsed[0].session_type
        {
            assert_eq!(host, "198.51.100.88");
            assert_eq!(port, 22022);
            assert_eq!(user, "devops");
            assert_eq!(*auth_method, AuthMethod::PrivateKey);
            assert!(key_path.as_deref().unwrap_or("").ends_with("my edge key.pem"));
            assert_eq!(jump_host.as_deref(), Some("bastion_user@198.51.100.1:2222"));
            assert!(tunnel_x11);
        } else {
            panic!("Expected SSH session for web-edge");
        }

        // 2. direct-node inherits bottom `Host *` User/Port, and `ProxyJump none` disables jump host
        assert_eq!(parsed[1].name, "direct-node");
        assert!(!parsed[1].use_ssh_gateway);
        if let SessionType::Ssh {
            ref host,
            port,
            ref user,
            ref auth_method,
            ref jump_host,
            ..
        } = parsed[1].session_type
        {
            assert_eq!(host, "192.0.2.55");
            assert_eq!(port, 2200);
            assert_eq!(user, "fallback_admin");
            assert_eq!(*auth_method, AuthMethod::Password);
            assert!(jump_host.is_none());
        } else {
            panic!("Expected SSH session for direct-node");
        }
    }

    #[test]
    fn test_looks_like_openssh_config_detection_and_dedup() {
        assert!(looks_like_openssh_config("Host my-server\n  HostName 1.2.3.4"));
        assert!(looks_like_openssh_config("  Host = prod-db\n  User = postgres"));
        assert!(!looks_like_openssh_config("root@1.2.3.4 = secret"));
        assert!(!looks_like_openssh_config("Server1=#109#0%1.2.3.4%22%root"));

        let cfg = "Host s1\n  HostName 10.0.0.9\n  User root\nHost s1-alias\n  HostName 10.0.0.9\n  User root\n";
        let imported = parse_sessions_from_text(cfg, "OpenSSH");
        assert_eq!(imported.len(), 2);

        // Merging two aliases pointing to the exact same (host, port, user) deduplicates the second one
        let mut existing = Vec::new();
        let summary = merge_imported_sessions(&mut existing, imported);
        assert_eq!(summary.total_parsed, 2);
        assert_eq!(summary.new_imported, 1);
        assert_eq!(summary.duplicates_skipped, 1);
    }
}
