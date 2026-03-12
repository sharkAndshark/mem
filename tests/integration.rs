use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn get_process_memory_kb(pid: u32) -> Option<usize> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "rss="])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.trim().parse::<usize>().ok()
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn get_process_memory_kb(pid: u32) -> Option<usize> {
    let script = format!(
        "Get-Process -Id {} | Select-Object -ExpandProperty WorkingSet64",
        pid
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let bytes: usize = stdout.trim().parse().ok()?;
        Some(bytes / 1024)
    } else {
        None
    }
}

fn get_binary_path() -> String {
    option_env!("CARGO_BIN_EXE_mem")
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                if cfg!(debug_assertions) {
                    "./target/debug/mem.exe".to_string()
                } else {
                    "./target/release/mem.exe".to_string()
                }
            } else if cfg!(debug_assertions) {
                "./target/debug/mem".to_string()
            } else {
                "./target/release/mem".to_string()
            }
        })
}

#[test]
fn test_help() {
    let binary = get_binary_path();
    let output = Command::new(&binary)
        .arg("--help")
        .output()
        .expect("Failed to execute mem --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Consume CPU and memory resources"));
}

#[test]
fn test_cli_runs_with_dynamic_inputs() {
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .args(["-c", "50%", "-m", "1G", "-d", "1"])
        .output()
        .expect("Failed to execute mem with dynamic args");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CPU:"));
    assert!(stdout.contains("Memory:"));
}

#[test]
fn test_duration_exits_in_expected_window() {
    let binary = get_binary_path();
    let start = std::time::Instant::now();

    let output = Command::new(&binary)
        .args(["-c", "0", "-m", "0", "-d", "2"])
        .output()
        .expect("Failed to execute duration test");

    assert!(output.status.success());

    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_secs(2));
    assert!(elapsed < Duration::from_secs(8));
}

#[test]
fn test_status_line_appears_for_longer_run() {
    let binary = get_binary_path();
    let output = Command::new(&binary)
        .args(["-c", "0", "-m", "64M", "-d", "6"])
        .output()
        .expect("Failed to execute status line test");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[Running] CPU:"));
}

#[test]
fn test_memory_allocation_reaches_reasonable_floor() {
    let binary = get_binary_path();
    let mut child = Command::new(&binary)
        .args(["-c", "0", "-m", "100M", "-d", "10"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn mem process");

    let pid = child.id();
    thread::sleep(Duration::from_secs(3));

    let memory_kb = get_process_memory_kb(pid).expect("Failed to read process memory");
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        memory_kb >= 40 * 1024,
        "Memory usage too low for 100M target: {} KB",
        memory_kb
    );
}

#[test]
fn test_zero_cpu_zero_memory_mode() {
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .args(["-c", "0", "-m", "0", "-d", "1"])
        .output()
        .expect("Failed to execute zero mode test");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CPU: none"));
    assert!(stdout.contains("Memory: none"));
}
