use clap::Args;
use std::path::PathBuf;

use crate::output;

#[derive(Args, Debug)]
pub struct DaemonArgs {
    /// Seconds between port scans (default: 2)
    #[arg(short, long, default_value = "2")]
    pub interval: u64,
    /// Install as launchd service (macOS) or systemd (Linux)
    #[arg(long)]
    pub install: bool,
    /// Remove installed service
    #[arg(long)]
    pub uninstall: bool,
}

pub fn run(args: DaemonArgs) -> anyhow::Result<()> {
    if args.install {
        install_service()?;
        return Ok(());
    }
    if args.uninstall {
        uninstall_service()?;
        return Ok(());
    }

    output::print_success(format!("Starting daemon (interval: {}s)...", args.interval));

    let mut daemon = libportier::daemon::Daemon::new(args.interval);
    match daemon.run() {
        Ok(_) => {}
        Err(e) => output::print_error(format!("Daemon error: {}", e)),
    }
    Ok(())
}

fn install_service() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let bin_path = std::env::current_exe()?;
        let plist_path = dirs_home()
            .ok_or_else(|| anyhow::anyhow!("HOME not set"))?
            .join("Library/LaunchAgents/com.portier.daemon.plist");

        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.portier.daemon</string>
<key>ProgramArguments</key><array><string>{}</string><string>daemon</string></array>
<key>KeepAlive</key><true/>
<key>RunAtLoad</key><true/>
<key>StandardOutPath</key><string>/tmp/portier-daemon.log</string>
<key>StandardErrorPath</key><string>/tmp/portier-daemon.log</string>
</dict></plist>"#,
            bin_path.to_string_lossy()
        );

        std::fs::write(&plist_path, &plist)?;
        output::print_success("Installed launchd service. Run: launchctl load ~/Library/LaunchAgents/com.portier.daemon.plist");
    }

    #[cfg(target_os = "linux")]
    {
        let bin_path = std::env::current_exe()?;
        let service = format!(
            r#"[Unit]
Description=Portier Port Manager Daemon
After=network.target

[Service]
ExecStart={} daemon
Restart=always

[Install]
WantedBy=default.target"#,
            bin_path.to_string_lossy()
        );

        let service_path = dirs_home()
            .ok_or_else(|| anyhow::anyhow!("HOME not set"))?
            .join(".config/systemd/user/portier-daemon.service");

        if let Some(parent) = service_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&service_path, &service)?;
        output::print_success(
            "Installed systemd user service. Run: systemctl --user enable --now portier-daemon",
        );
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        output::print_warning(
            "Auto-install not supported on this OS. Run `portier daemon` manually.",
        );
    }

    Ok(())
}

fn uninstall_service() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let plist_path = dirs_home()
            .ok_or_else(|| anyhow::anyhow!("HOME not set"))?
            .join("Library/LaunchAgents/com.portier.daemon.plist");
        if plist_path.exists() {
            std::fs::remove_file(&plist_path)?;
            output::print_success("Removed launchd service.");
        } else {
            output::print_warning("No launchd service found.");
        }
    }

    #[cfg(target_os = "linux")]
    {
        let service_path = dirs_home()
            .ok_or_else(|| anyhow::anyhow!("HOME not set"))?
            .join(".config/systemd/user/portier-daemon.service");
        if service_path.exists() {
            std::fs::remove_file(&service_path)?;
            output::print_success("Removed systemd service.");
        } else {
            output::print_warning("No systemd service found.");
        }
    }

    Ok(())
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}
