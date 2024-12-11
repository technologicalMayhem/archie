use std::process::Command;

fn main() {
    let Ok(describe_out) = Command::new("git").args(["describe", "--dirty", "--broken", "--tags"]).output() else {
        eprintln!("Failed to run 'git describe'");
        return;
    };
    let describe = String::from_utf8_lossy(&describe_out.stdout);
    println!("cargo::rustc-env=APP_VERSION={describe}");
}