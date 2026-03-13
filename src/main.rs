mod cpu;
mod memory;
mod metrics;

use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const STATUS_INTERVAL_SECS: u64 = 5;

#[derive(Parser, Debug)]
#[command(name = "mem")]
#[command(about = "Dynamic CPU/Memory stress tool (Linux/Windows)")]
struct Args {
    #[arg(short, long, value_name = "PERCENT")]
    cpu: String,

    #[arg(short, long, value_name = "PERCENT")]
    memory: String,
}

fn parse_percent(input: &str) -> Option<u32> {
    let s = input.trim();
    let value = s.strip_suffix('%')?.trim().parse::<u32>().ok()?;
    if value <= 100 {
        Some(value)
    } else {
        None
    }
}

fn bytes_to_gb(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

fn get_total_memory_bytes() -> usize {
    sys_info::mem_info()
        .map(|m| m.total as usize * 1024)
        .unwrap_or(8 * 1024 * 1024 * 1024)
}

fn main() {
    let args = Args::parse();

    let cpu_percent = match parse_percent(&args.cpu) {
        Some(v) => v,
        None => {
            eprintln!("Invalid CPU percent '{}'. Use format like -c 50%", args.cpu);
            std::process::exit(2);
        }
    };

    let memory_percent = match parse_percent(&args.memory) {
        Some(v) => v,
        None => {
            eprintln!(
                "Invalid memory percent '{}'. Use format like -m 60%",
                args.memory
            );
            std::process::exit(2);
        }
    };

    let running = Arc::new(AtomicBool::new(true));
    let signal_flag = Arc::clone(&running);
    ctrlc::set_handler(move || {
        signal_flag.store(false, Ordering::Relaxed);
    })
    .expect("failed to set Ctrl-C handler");

    let total_memory = get_total_memory_bytes();
    let target_memory = total_memory.saturating_mul(memory_percent as usize) / 100;

    let cpu_controller = cpu::CpuController::new(cpu_percent, Arc::clone(&running));
    cpu_controller.set_target_percent(cpu_percent);
    cpu_controller.start();

    let mut memory_controller = memory::MemoryController::new(target_memory);
    memory_controller.set_target(target_memory);

    println!(
        "Start: CPU target {}%, MEM target {}% ({:.2} GB)",
        cpu_percent,
        memory_percent,
        bytes_to_gb(target_memory)
    );
    println!("Press Ctrl-C to stop");

    let mut last_status = Instant::now();
    let mut last_cpu_sample_at = Instant::now();
    let mut last_cpu_micros = metrics::process_cpu_time_micros().unwrap_or(0);
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get() as f64)
        .unwrap_or(1.0)
        .max(1.0);

    while running.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(250));

        let observed_private =
            metrics::process_private_bytes().unwrap_or_else(|| memory_controller.allocated_bytes());
        memory_controller.step(observed_private, Arc::clone(&running));
        memory_controller.touch_hot_pages();

        if last_cpu_sample_at.elapsed() >= Duration::from_secs(1) {
            let now_cpu_micros = metrics::process_cpu_time_micros().unwrap_or(last_cpu_micros);
            let delta_cpu = now_cpu_micros.saturating_sub(last_cpu_micros) as f64;
            let elapsed_wall = last_cpu_sample_at.elapsed().as_micros() as f64;
            if elapsed_wall > 0.0 {
                let observed_percent = (delta_cpu / elapsed_wall / cpu_count) * 100.0;
                cpu_controller.update_from_observed(observed_percent);
            }
            last_cpu_micros = now_cpu_micros;
            last_cpu_sample_at = Instant::now();
        }

        if last_status.elapsed() >= Duration::from_secs(STATUS_INTERVAL_SECS) {
            last_status = Instant::now();
            let actual = metrics::process_private_bytes().unwrap_or(observed_private);
            let alloc = memory_controller.allocated_bytes();

            println!(
                "[Running] CPU target: {}% | CPU duty: {}% | MEM private: {:.2} GB ({:.0}%) | MEM alloc: {:.2} GB",
                cpu_controller.get_target_percent(),
                cpu_controller.get_duty_percent(),
                bytes_to_gb(actual),
                (actual as f64 / total_memory as f64 * 100.0).round(),
                bytes_to_gb(alloc)
            );
        }
    }

    println!("Stopped.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_percent_valid() {
        assert_eq!(parse_percent("0%"), Some(0));
        assert_eq!(parse_percent("50%"), Some(50));
        assert_eq!(parse_percent("100%"), Some(100));
        assert_eq!(parse_percent(" 25% "), Some(25));
    }

    #[test]
    fn test_parse_percent_invalid() {
        assert_eq!(parse_percent("101%"), None);
        assert_eq!(parse_percent("50"), None);
        assert_eq!(parse_percent("abc%"), None);
        assert_eq!(parse_percent("-1%"), None);
    }
}
