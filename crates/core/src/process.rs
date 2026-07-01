use std::path::Path;
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
    match Path::new(program)
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
                .arg(program)
                .args(args);
            command
        }
        Some("cmd") | Some("bat") => {
            let mut command = Command::new("cmd.exe");
            command.arg("/d").arg("/s").arg("/c").arg(program).args(args);
            command
        }
        _ => {
            let mut command = Command::new(program);
            command.args(args);
            command
        }
    }
}
