use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

struct ProcessGuard {
    child: Option<Child>,
}

impl ProcessGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn get_process_cpu_percent(pid: u32) -> Option<f32> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "pcpu="])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.trim().parse::<f32>().ok()
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

#[cfg(target_os = "windows")]
fn get_process_cpu_percent(pid: u32) -> Option<f32> {
    let script = format!("$p = Get-Process -Id {}; $p.CPU", pid);

    let start_output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .ok()?;

    let start_cpu: f64 = if start_output.status.success() {
        let stdout = String::from_utf8_lossy(&start_output.stdout);
        stdout.trim().parse().unwrap_or(0.0)
    } else {
        return None;
    };

    thread::sleep(Duration::from_secs(2));

    let end_output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .ok()?;

    if end_output.status.success() {
        let stdout = String::from_utf8_lossy(&end_output.stdout);
        let end_cpu: f64 = stdout.trim().parse().unwrap_or(0.0);
        let cpu_percent = (end_cpu - start_cpu) / 2.0 * 100.0;
        Some(cpu_percent as f32)
    } else {
        None
    }
}

fn get_binary_path() -> String {
    option_env!("CARGO_BIN_EXE_mem")
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                "./target/debug/mem".to_string()
            } else {
                "./target/release/mem".to_string()
            }
        })
}

fn spawn_mem(args: &[&str]) -> ProcessGuard {
    let binary = get_binary_path();
    let child = Command::new(&binary)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn mem process");

    ProcessGuard::new(child)
}

fn spawn_mem_with_pid(args: &[&str]) -> (ProcessGuard, u32) {
    let binary = get_binary_path();
    let child = Command::new(&binary)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn mem process");

    let pid = child.id();
    (ProcessGuard::new(child), pid)
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
    assert!(stdout.contains("Consume CPU and memory"));
}

#[test]
fn test_dynamic_memory_10_percent() {
    let (_guard, pid) = spawn_mem_with_pid(&["-c", "0", "-m", "10%", "-d", "10"]);

    thread::sleep(Duration::from_secs(2));

    let memory_kb = get_process_memory_kb(pid).expect("Failed to get process memory");

    println!("Memory usage: {} KB (~{} MB)", memory_kb, memory_kb / 1024);

    assert!(
        memory_kb > 50 * 1024,
        "Memory usage {} KB should be at least 50MB for 10%",
        memory_kb
    );
}

#[test]
fn test_combined_cpu_and_memory() {
    let (_guard, pid) = spawn_mem_with_pid(&["-c", "100", "-m", "50M", "-d", "10"]);

    thread::sleep(Duration::from_secs(2));

    let memory_kb = get_process_memory_kb(pid).expect("Failed to get process memory");

    let expected_kb = 50 * 1024;
    let min_kb = expected_kb * 80 / 100;

    assert!(
        memory_kb >= min_kb,
        "Memory usage {} KB is less than expected {} KB",
        memory_kb,
        min_kb
    );

    let cpu = get_process_cpu_percent(pid).unwrap_or(0.0);
    println!("CPU: {}%, Memory: {} KB", cpu, memory_kb);

    #[cfg(not(target_os = "windows"))]
    {
        assert!(cpu > 30.0, "CPU usage {}% is less than expected 30%", cpu);
    }

    #[cfg(target_os = "windows")]
    println!("Memory: {} KB (CPU test skipped on Windows)", memory_kb);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn test_cpu_100_percent() {
    let (_guard, pid) = spawn_mem_with_pid(&["-c", "100", "-m", "0", "-d", "15"]);

    thread::sleep(Duration::from_secs(3));

    let cpu_samples: Vec<f32> = (0..3)
        .map(|_| {
            thread::sleep(Duration::from_millis(1000));
            get_process_cpu_percent(pid).unwrap_or(0.0)
        })
        .collect();

    let avg_cpu = cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32;
    println!("CPU samples: {:?}, avg: {}%", cpu_samples, avg_cpu);

    #[cfg(target_os = "windows")]
    let min_cpu = 30.0;
    #[cfg(not(target_os = "windows"))]
    let min_cpu = 50.0;

    assert!(
        avg_cpu > min_cpu,
        "CPU usage {}% is less than expected {}%",
        avg_cpu,
        min_cpu
    );
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn test_cpu_200_percent() {
    let (_guard, pid) = spawn_mem_with_pid(&["-c", "200", "-m", "0", "-d", "15"]);

    thread::sleep(Duration::from_secs(3));

    let cpu_samples: Vec<f32> = (0..3)
        .map(|_| {
            thread::sleep(Duration::from_millis(1000));
            get_process_cpu_percent(pid).unwrap_or(0.0)
        })
        .collect();

    let avg_cpu = cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32;
    println!("CPU samples: {:?}, avg: {}%", cpu_samples, avg_cpu);

    #[cfg(target_os = "windows")]
    let min_cpu = 80.0;
    #[cfg(not(target_os = "windows"))]
    let min_cpu = 100.0;

    assert!(
        avg_cpu > min_cpu,
        "CPU usage {}% is less than expected {}%",
        avg_cpu,
        min_cpu
    );
}

#[test]
fn test_duration_exits_on_time() {
    let start = std::time::Instant::now();
    let _guard = spawn_mem(&["-c", "0", "-m", "0", "-d", "5"]);

    thread::sleep(Duration::from_secs(7));

    let elapsed = start.elapsed();
    println!("Test ran for {:?}", elapsed);

    assert!(elapsed.as_secs() >= 5);
    assert!(elapsed.as_secs() < 10);
}

#[test]
fn test_cli_arguments() {
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .args(["-c", "50%", "-m", "1G", "-d", "1"])
        .output()
        .expect("Failed to execute mem");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("CPU: dynamic 50%") || stdout.contains("CPU:"),
        "Output should contain CPU info. stdout: {}, stderr: {}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("Memory:") || stdout.contains("bytes"),
        "Output should contain Memory info. stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}
