use crate::types::TerminalInfo;

#[cfg(target_os = "macos")]
pub fn detect_terminals() -> Vec<TerminalInfo> {
    let mut terminals = Vec::new();

    let candidates = [
        ("iTerm2", "/Applications/iTerm.app/Contents/MacOS/iTerm2"),
        ("Warp", "/Applications/Warp.app/Contents/MacOS/warp"),
        ("Alacritty", "/Applications/Alacritty.app/Contents/MacOS/alacritty"),
        ("kitty", "/Applications/kitty.app/Contents/MacOS/kitty"),
        ("Terminal", "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal"),
    ];

    for (name, path) in &candidates {
        if std::path::Path::new(path).exists() {
            terminals.push(TerminalInfo {
                name: name.to_string(),
                path: path.to_string(),
                is_preferred: false,
            });
        }
    }

    // Always add Terminal.app as fallback
    if terminals.is_empty() {
        terminals.push(TerminalInfo {
            name: "Terminal".to_string(),
            path: "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal".to_string(),
            is_preferred: false,
        });
    }

    terminals
}

#[cfg(target_os = "linux")]
pub fn detect_terminals() -> Vec<TerminalInfo> {
    let mut terminals = Vec::new();

    // Check $TERMINAL env var
    if let Ok(term) = std::env::var("TERMINAL") {
        if std::path::Path::new(&term).exists() {
            terminals.push(TerminalInfo {
                name: term.clone(),
                path: term,
                is_preferred: true,
            });
        }
    }

    let candidates = [
        ("gnome-terminal", "gnome-terminal"),
        ("konsole", "konsole"),
        ("xfce4-terminal", "xfce4-terminal"),
        ("alacritty", "alacritty"),
        ("kitty", "kitty"),
        ("xterm", "xterm"),
    ];

    for (name, cmd) in &candidates {
        if which_terminal(cmd) {
            terminals.push(TerminalInfo {
                name: name.to_string(),
                path: cmd.to_string(),
                is_preferred: false,
            });
        }
    }

    terminals
}

#[cfg(target_os = "windows")]
pub fn detect_terminals() -> Vec<TerminalInfo> {
    let mut terminals = Vec::new();

    let wt_paths = [
        r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.0.0.0_x64__8wekyb3d8bbwe\wt.exe",
        r"C:\Users\Default\AppData\Local\Microsoft\WindowsApps\wt.exe",
    ];

    for wt in &wt_paths {
        if std::path::Path::new(wt).exists() {
            terminals.push(TerminalInfo {
                name: "Windows Terminal".to_string(),
                path: wt.to_string(),
                is_preferred: false,
            });
            break;
        }
    }

    terminals.push(TerminalInfo {
        name: "PowerShell".to_string(),
        path: "powershell.exe".to_string(),
        is_preferred: false,
    });

    terminals.push(TerminalInfo {
        name: "Command Prompt".to_string(),
        path: "cmd.exe".to_string(),
        is_preferred: false,
    });

    terminals
}

#[cfg(target_os = "linux")]
fn which_terminal(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Build the command args to launch ssh in a given terminal
pub fn build_launch_args(terminal_name: &str, ssh_command: &str) -> (String, Vec<String>) {
    #[cfg(target_os = "macos")]
    {
        match terminal_name {
            "iTerm2" => {
                let script = format!(
                    r#"tell application "iTerm2"
                        create window with default profile
                        tell current session of current window
                            write text "{}"
                        end tell
                    end tell"#,
                    ssh_command.replace('"', r#"\""#)
                );
                ("osascript".to_string(), vec!["-e".to_string(), script])
            }
            "Terminal" => {
                let script = format!(
                    r#"tell application "Terminal"
                        do script "{}"
                        activate
                    end tell"#,
                    ssh_command.replace('"', r#"\""#)
                );
                ("osascript".to_string(), vec!["-e".to_string(), script])
            }
            "Warp" => (
                "open".to_string(),
                vec!["-a".to_string(), "Warp".to_string(), "--args".to_string(), ssh_command.to_string()],
            ),
            "Alacritty" => (
                "alacritty".to_string(),
                vec!["-e".to_string(), "sh".to_string(), "-c".to_string(), ssh_command.to_string()],
            ),
            "kitty" => (
                "kitty".to_string(),
                vec!["sh".to_string(), "-c".to_string(), ssh_command.to_string()],
            ),
            _ => {
                // Fallback: Terminal.app AppleScript
                let script = format!(
                    r#"tell application "Terminal"
                        do script "{}"
                        activate
                    end tell"#,
                    ssh_command.replace('"', r#"\""#)
                );
                ("osascript".to_string(), vec!["-e".to_string(), script])
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        match terminal_name {
            "gnome-terminal" => (
                "gnome-terminal".to_string(),
                vec!["--".to_string(), "sh".to_string(), "-c".to_string(), ssh_command.to_string()],
            ),
            "konsole" => (
                "konsole".to_string(),
                vec!["-e".to_string(), ssh_command.to_string()],
            ),
            "xfce4-terminal" => (
                "xfce4-terminal".to_string(),
                vec!["--command".to_string(), ssh_command.to_string()],
            ),
            name => (
                name.to_string(),
                vec!["-e".to_string(), ssh_command.to_string()],
            ),
        }
    }

    #[cfg(target_os = "windows")]
    {
        match terminal_name {
            "Windows Terminal" => (
                "wt.exe".to_string(),
                vec!["new-tab".to_string(), "--".to_string(), "powershell.exe".to_string(), "-NoExit".to_string(), "-Command".to_string(), ssh_command.to_string()],
            ),
            "PowerShell" => (
                "powershell.exe".to_string(),
                vec!["-NoExit".to_string(), "-Command".to_string(), ssh_command.to_string()],
            ),
            _ => (
                "cmd.exe".to_string(),
                vec!["/K".to_string(), ssh_command.to_string()],
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    mod macos {
        use super::*;

        #[test]
        fn iterm2_uses_osascript_with_iterm_script() {
            let (program, args) = build_launch_args("iTerm2", "ssh myserver");
            assert_eq!(program, "osascript");
            let script = args.last().unwrap();
            assert!(script.contains("iTerm2"));
            assert!(script.contains("ssh myserver"));
        }

        #[test]
        fn terminal_app_uses_do_script() {
            let (program, args) = build_launch_args("Terminal", "ssh myserver");
            assert_eq!(program, "osascript");
            let script = args.last().unwrap();
            assert!(script.contains("do script"));
            assert!(script.contains("ssh myserver"));
        }

        #[test]
        fn alacritty_uses_sh_dash_c() {
            let (program, args) = build_launch_args("Alacritty", "ssh myserver");
            assert_eq!(program, "alacritty");
            assert!(args.contains(&"-e".to_string()));
            assert!(args.contains(&"sh".to_string()));
            assert!(args.contains(&"-c".to_string()));
            assert!(args.contains(&"ssh myserver".to_string()));
        }

        #[test]
        fn kitty_uses_sh_dash_c() {
            let (program, args) = build_launch_args("kitty", "ssh myserver");
            assert_eq!(program, "kitty");
            assert!(args.contains(&"sh".to_string()));
            assert!(args.contains(&"-c".to_string()));
            assert!(args.contains(&"ssh myserver".to_string()));
        }

        #[test]
        fn warp_uses_open_dash_a() {
            let (program, args) = build_launch_args("Warp", "ssh myserver");
            assert_eq!(program, "open");
            assert!(args.contains(&"-a".to_string()));
            assert!(args.contains(&"Warp".to_string()));
        }

        #[test]
        fn unknown_terminal_falls_back_to_terminal_app() {
            let (program, args) = build_launch_args("SomeUnknownTerm", "ssh myserver");
            assert_eq!(program, "osascript");
            let script = args.last().unwrap();
            assert!(script.contains("Terminal"));
        }
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use super::*;

        #[test]
        fn gnome_terminal_uses_double_dash() {
            let (program, args) = build_launch_args("gnome-terminal", "ssh myserver");
            assert_eq!(program, "gnome-terminal");
            assert!(args.contains(&"--".to_string()));
        }

        #[test]
        fn konsole_uses_dash_e() {
            let (program, args) = build_launch_args("konsole", "ssh myserver");
            assert_eq!(program, "konsole");
            assert_eq!(args[0], "-e");
        }
    }
}
