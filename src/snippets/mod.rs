use serde::{Deserialize, Serialize};
use regex::Regex;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub description: String,
}

fn default_category() -> String {
    "General".to_string()
}

pub fn default_snippets() -> Vec<Snippet> {
    vec![
        // Docker
        Snippet {
            id: "dock_ps".into(),
            name: "Docker: List Containers".into(),
            command: "docker ps -a".into(),
            category: "Docker".into(),
            description: "Show all running and stopped containers".into(),
        },
        Snippet {
            id: "dock_logs".into(),
            name: "Docker: Follow Logs".into(),
            command: "docker logs -f --tail 100 {{container_id}}".into(),
            category: "Docker".into(),
            description: "Follow live logs with last 100 lines".into(),
        },
        Snippet {
            id: "dock_stats".into(),
            name: "Docker: Live Resource Stats".into(),
            command: "docker stats".into(),
            category: "Docker".into(),
            description: "Display live stream of container CPU, RAM, I/O".into(),
        },
        Snippet {
            id: "dock_compose_up".into(),
            name: "Docker Compose: Up Background".into(),
            command: "docker compose up -d".into(),
            category: "Docker".into(),
            description: "Build, (re)create, and start containers in background".into(),
        },
        Snippet {
            id: "dock_prune".into(),
            name: "Docker: System Prune".into(),
            command: "docker system prune -af".into(),
            category: "Docker".into(),
            description: "Remove all unused containers, networks, images".into(),
        },

        // Nginx & Web
        Snippet {
            id: "ngx_test_reload".into(),
            name: "Nginx: Test & Reload".into(),
            command: "nginx -t && systemctl reload nginx".into(),
            category: "Nginx".into(),
            description: "Check configuration syntax and gracefully reload Nginx".into(),
        },
        Snippet {
            id: "ngx_err_log".into(),
            name: "Nginx: Error Log Stream".into(),
            command: "tail -f /var/log/nginx/error.log".into(),
            category: "Nginx".into(),
            description: "Watch Nginx error log in real-time".into(),
        },
        Snippet {
            id: "ngx_access_log".into(),
            name: "Nginx: Access Log Stream".into(),
            command: "tail -f /var/log/nginx/access.log".into(),
            category: "Nginx".into(),
            description: "Watch incoming HTTP requests".into(),
        },
        Snippet {
            id: "ssl_certbot".into(),
            name: "SSL: Certbot Renew Dry-Run".into(),
            command: "certbot renew --dry-run".into(),
            category: "Nginx".into(),
            description: "Test Let's Encrypt SSL certificate renewal".into(),
        },

        // Linux System & Diagnostics
        Snippet {
            id: "sys_htop".into(),
            name: "System: Process Monitor (htop)".into(),
            command: "htop".into(),
            category: "System".into(),
            description: "Interactive process viewer and manager".into(),
        },
        Snippet {
            id: "sys_df".into(),
            name: "System: Disk Usage (df -h)".into(),
            command: "df -h".into(),
            category: "System".into(),
            description: "Human-readable filesystem disk space".into(),
        },
        Snippet {
            id: "sys_free".into(),
            name: "System: Memory Usage (free -m)".into(),
            command: "free -m".into(),
            category: "System".into(),
            description: "RAM and swap memory in megabytes".into(),
        },
        Snippet {
            id: "sys_ports".into(),
            name: "System: Listening Ports (netstat)".into(),
            command: "netstat -tlpn".into(),
            category: "System".into(),
            description: "Show listening TCP/UDP sockets and process IDs".into(),
        },
        Snippet {
            id: "sys_journal".into(),
            name: "System: Service Log (journalctl)".into(),
            command: "journalctl -u {{service_name}} -f -n 50".into(),
            category: "System".into(),
            description: "Follow systemd unit service logs".into(),
        },
        Snippet {
            id: "sys_restart_service".into(),
            name: "System: Restart Service".into(),
            command: "systemctl restart {{service_name}}".into(),
            category: "System".into(),
            description: "Restart a systemd service".into(),
        },

        // Database
        Snippet {
            id: "db_mysql_cli".into(),
            name: "MySQL: Login CLI".into(),
            command: "mysql -u {{username}} -p".into(),
            category: "Database".into(),
            description: "Connect to MySQL / MariaDB server".into(),
        },
        Snippet {
            id: "db_mysql_dump".into(),
            name: "MySQL: Export Database".into(),
            command: "mysqldump -u {{username}} -p {{database_name}} > {{output_file.sql}}".into(),
            category: "Database".into(),
            description: "Dump database schema and data to SQL file".into(),
        },
        Snippet {
            id: "db_redis_cli".into(),
            name: "Redis: Server Info".into(),
            command: "redis-cli info".into(),
            category: "Database".into(),
            description: "Get Redis metrics, memory and connected clients".into(),
        },

        // Git & Deployment
        Snippet {
            id: "git_status".into(),
            name: "Git: Status".into(),
            command: "git status".into(),
            category: "Git".into(),
            description: "Check modified files and branch status".into(),
        },
        Snippet {
            id: "git_pull".into(),
            name: "Git: Pull Branch".into(),
            command: "git pull origin {{branch}}".into(),
            category: "Git".into(),
            description: "Fetch and merge changes from remote branch".into(),
        },
        Snippet {
            id: "git_log".into(),
            name: "Git: Recent Commits".into(),
            command: "git log --oneline -n 10".into(),
            category: "Git".into(),
            description: "Show 10 most recent commits in compact format".into(),
        },
    ]
}

/// Extracts all `{{placeholder}}` variables from command string
pub fn extract_placeholders(cmd: &str) -> Vec<String> {
    let re = Regex::new(r"\{\{([a-zA-Z0-9_\-\.]+)\}\}").unwrap();
    let mut vars = Vec::new();
    for cap in re.captures_iter(cmd) {
        if let Some(m) = cap.get(1) {
            let var = m.as_str().to_string();
            if !vars.contains(&var) {
                vars.push(var);
            }
        }
    }
    vars
}

/// Replaces all `{{key}}` with corresponding value from list
pub fn replace_placeholders(cmd: &str, values: &[(String, String)]) -> String {
    let mut result = cmd.to_string();
    for (key, val) in values {
        let pattern = format!("{{{{{}}}}}", key);
        result = result.replace(&pattern, val);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_and_replace_placeholders() {
        let cmd = "docker logs -f --tail 100 {{container_id}} && echo {{container_id}}";
        let vars = extract_placeholders(cmd);
        assert_eq!(vars, vec!["container_id"]);

        let replaced = replace_placeholders(cmd, &[("container_id".into(), "my-nginx-app".into())]);
        assert_eq!(replaced, "docker logs -f --tail 100 my-nginx-app && echo my-nginx-app");
    }

    #[test]
    fn test_default_snippets_non_empty_and_valid() {
        let snippets = default_snippets();
        assert!(snippets.len() >= 15);
        for s in &snippets {
            assert!(!s.id.is_empty());
            assert!(!s.name.is_empty());
            assert!(!s.command.is_empty());
            assert!(!s.category.is_empty());
        }
    }
}
