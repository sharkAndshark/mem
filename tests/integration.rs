use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn get_binary_path() -> String {
    option_env!("CARGO_BIN_EXE_mem")
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "./target/debug/mem.exe".to_string()
            } else {
                "./target/debug/mem".to_string()
            }
        })
}

#[test]
fn test_help() {
    let bin = get_binary_path();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dynamic CPU/Memory stress tool"));
}

#[test]
fn test_rejects_non_percent_memory() {
    let bin = get_binary_path();
    let output = Command::new(&bin)
        .args(["-c", "20%", "-m", "1G"])
        .output()
        .expect("failed to run command");

    assert!(!output.status.success());
}

#[test]
fn test_rejects_non_percent_cpu() {
    let bin = get_binary_path();
    let output = Command::new(&bin)
        .args(["-c", "100", "-m", "10%"])
        .output()
        .expect("failed to run command");

    assert!(!output.status.success());
}

#[test]
fn test_repeated_status_lines_appear() {
    let bin = get_binary_path();
    let mut child = Command::new(&bin)
        .args(["-c", "0%", "-m", "1%"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn command");

    thread::sleep(Duration::from_secs(7));
    let _ = child.kill();
    let output = child.wait_with_output().expect("failed to collect output");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let running_lines = stdout.matches("[Running]").count();
    assert!(
        running_lines >= 1,
        "expected at least one running line, got: {stdout}"
    );
}
