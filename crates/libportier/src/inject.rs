//! Framework-aware environment-variable injection for `portier run`.
//!
//! When `portier run` allocates a free port for a child dev server, it injects
//! the port via environment variables so the server binds it without the user
//! editing any config. `PORT` is honored by Node, Next.js, CRA, Nuxt, Rails,
//! and most generic servers. Vite reads its port from config rather than `PORT`,
//! so we also export `VITE_PORT` as a hint when the command looks like Vite.

/// Build the `(key, value)` env pairs to inject for `command` on `port`.
pub fn framework_env_vars(command: &[String], port: u16) -> Vec<(String, String)> {
    let mut vars = vec![("PORT".to_string(), port.to_string())];

    let joined = command.join(" ").to_lowercase();
    if joined.contains("vite") {
        vars.push(("VITE_PORT".to_string(), port.to_string()));
    }

    vars
}

/// Rewrite the port argument for servers that take the port as a CLI flag
/// rather than `PORT` (Next.js `-p`, Vite `--port`, Django `runserver`).
///
/// Returns `Some(modified_command)` when the command is a recognized arg-style
/// server, or `None` to leave the command untouched (env injection still
/// applies). Only the canonical, directly-invoked forms are matched — a port
/// hidden inside an `npm run dev` script can't be seen and is left alone.
pub fn apply_port_to_args(command: &[String], port: u16) -> Option<Vec<String>> {
    if command.is_empty() {
        return None;
    }
    let joined = command.join(" ").to_lowercase();
    let value = port.to_string();

    if joined.contains("next dev") {
        return Some(set_or_append_flag(command, &["-p", "--port"], &value));
    }
    if joined.contains("manage.py runserver") {
        return Some(set_django_runserver(command, port));
    }
    if command.iter().any(|a| a == "vite") {
        return Some(set_or_append_flag(command, &["--port"], &value));
    }
    None
}

/// Set `value` for the first matching flag (handling both `--flag value` and
/// `--flag=value` forms), or append `flags[0] value` if no flag is present.
fn set_or_append_flag(command: &[String], flags: &[&str], value: &str) -> Vec<String> {
    let mut out = command.to_vec();
    for i in 0..out.len() {
        for f in flags {
            if out[i] == *f {
                if i + 1 < out.len() {
                    out[i + 1] = value.to_string();
                } else {
                    out.push(value.to_string());
                }
                return out;
            }
            let eq = format!("{f}=");
            if out[i].starts_with(&eq) {
                out[i] = format!("{f}={value}");
                return out;
            }
        }
    }
    out.push(flags[0].to_string());
    out.push(value.to_string());
    out
}

/// Replace the port in Django's `runserver [addr:]port` argument, or append
/// `0.0.0.0:<port>` if no address was given.
fn set_django_runserver(command: &[String], port: u16) -> Vec<String> {
    let mut out = command.to_vec();
    if let Some(rs) = out.iter().position(|a| a == "runserver") {
        for j in (rs + 1)..out.len() {
            if out[j].starts_with('-') {
                continue;
            }
            if let Some((host, _)) = out[j].rsplit_once(':') {
                out[j] = format!("{host}:{port}");
                return out;
            }
            if out[j].chars().all(|c| c.is_ascii_digit()) {
                out[j] = port.to_string();
                return out;
            }
        }
        out.push(format!("0.0.0.0:{port}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_sets_port() {
        let cmd = vec!["npm".to_string(), "run".to_string(), "dev".to_string()];
        let vars = framework_env_vars(&cmd, 3005);
        assert!(vars.contains(&("PORT".to_string(), "3005".to_string())));
    }

    #[test]
    fn test_vite_adds_vite_port() {
        let cmd = vec!["vite".to_string(), "--host".to_string()];
        let vars = framework_env_vars(&cmd, 5174);
        assert!(vars.contains(&("PORT".to_string(), "5174".to_string())));
        assert!(vars.contains(&("VITE_PORT".to_string(), "5174".to_string())));
    }

    #[test]
    fn test_non_vite_has_no_vite_port() {
        let cmd = vec!["node".to_string(), "server.js".to_string()];
        let vars = framework_env_vars(&cmd, 4000);
        assert!(!vars.iter().any(|(k, _)| k == "VITE_PORT"));
    }

    fn cmd(s: &str) -> Vec<String> {
        s.split(' ').map(String::from).collect()
    }

    #[test]
    fn test_next_replaces_existing_flag() {
        let out = apply_port_to_args(&cmd("next dev -p 3000"), 3005).unwrap();
        assert_eq!(out, cmd("next dev -p 3005"));
    }

    #[test]
    fn test_next_appends_flag_when_absent() {
        let out = apply_port_to_args(&cmd("next dev"), 3005).unwrap();
        assert_eq!(out, cmd("next dev -p 3005"));
    }

    #[test]
    fn test_vite_appends_long_flag() {
        let out = apply_port_to_args(&cmd("vite"), 5174).unwrap();
        assert_eq!(out, cmd("vite --port 5174"));
    }

    #[test]
    fn test_vite_replaces_eq_form() {
        let out = apply_port_to_args(&cmd("vite --port=5173"), 5174).unwrap();
        assert_eq!(out, cmd("vite --port=5174"));
    }

    #[test]
    fn test_django_replaces_address_port() {
        let out =
            apply_port_to_args(&cmd("python manage.py runserver 0.0.0.0:8000"), 8001).unwrap();
        assert_eq!(out, cmd("python manage.py runserver 0.0.0.0:8001"));
    }

    #[test]
    fn test_django_appends_when_no_address() {
        let out = apply_port_to_args(&cmd("python manage.py runserver"), 8001).unwrap();
        assert_eq!(out, cmd("python manage.py runserver 0.0.0.0:8001"));
    }

    #[test]
    fn test_django_bare_port() {
        let out = apply_port_to_args(&cmd("python manage.py runserver 8000"), 8001).unwrap();
        assert_eq!(out, cmd("python manage.py runserver 8001"));
    }

    #[test]
    fn test_unknown_command_left_alone() {
        assert!(apply_port_to_args(&cmd("node server.js"), 3000).is_none());
    }
}
