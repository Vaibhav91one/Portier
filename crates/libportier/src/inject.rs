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
}
