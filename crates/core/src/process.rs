use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

pub fn command_with_args(program: &str, args: &[String]) -> Command {
    let mut command = if cfg!(windows) {
        windows_command(program, args)
    } else {
        let mut command = Command::new(program);
        command.args(args);
        command
    };
    command.stdin(Stdio::null());
    command
}

pub fn command_with_stdio(program: &str, args: &[String]) -> Command {
    if cfg!(windows) {
        windows_command(program, args)
    } else {
        let mut command = Command::new(program);
        command.args(args);
        command
    }
}

fn windows_command(program: &str, args: &[String]) -> Command {
    let program = normalize_windows_program(program);
    match Path::new(&program)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("ps1") => {
            let mut command = Command::new("powershell.exe");
            command
                .arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(&program)
                .args(args);
            command
        }
        Some("cmd") | Some("bat") => {
            let mut command = Command::new("cmd.exe");
            command
                .arg("/d")
                .arg("/s")
                .arg("/c")
                .arg(&program)
                .args(args);
            command
        }
        _ => {
            let mut command = Command::new(&program);
            command.args(args);
            command
        }
    }
}

fn normalize_windows_program(program: &str) -> String {
    let path = Path::new(program);
    if path.extension().is_some() {
        return program.to_string();
    }

    if let Some(candidate) = sibling_with_windows_extension(path) {
        return candidate.display().to_string();
    }

    if let Ok(resolved) = which::which(program) {
        if let Some(candidate) = sibling_with_windows_extension(&resolved) {
            return candidate.display().to_string();
        }
        return resolved.display().to_string();
    }

    program.to_string()
}

fn sibling_with_windows_extension(path: &Path) -> Option<PathBuf> {
    if path.extension().is_some() {
        return Some(path.to_path_buf());
    }
    for extension in ["exe", "cmd", "bat", "ps1"] {
        let candidate = path.with_extension(extension);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn normalizes_extensionless_windows_shim_to_cmd() {
        let temp = tempfile::tempdir().unwrap();
        let shim = temp.path().join("opencode");
        let cmd = temp.path().join("opencode.cmd");
        std::fs::write(&shim, "").unwrap();
        std::fs::write(&cmd, "@echo off").unwrap();

        let normalized = super::normalize_windows_program(&shim.display().to_string());

        assert!(normalized.ends_with("opencode.cmd"));
    }
}
