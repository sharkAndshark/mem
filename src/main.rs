mod cpu;
mod memory;

use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const ADJUST_INTERVAL_SECS: u64 = 5;

#[derive(Debug, Clone)]
enum CpuTarget {
    Fixed(u32),
    Dynamic(f64),
    None,
}

#[derive(Debug, Clone)]
enum MemoryTarget {
    Fixed(usize),
    Dynamic(f64),
    None,
}

#[derive(Parser, Debug)]
#[command(name = "mem")]
#[command(about = "Consume CPU and memory resources", long_about = None)]
struct Args {
    #[arg(short, long, default_value = "1")]
    cpu: String,

    #[arg(short = 'm', long, default_value = "0")]
    memory: String,

    #[arg(short, long, default_value = "1")]
    duration: u64,
}

fn parse_cpu(s: &str) -> CpuTarget {
    let s = s.trim();
    if s.is_empty() || s == "0" {
        return CpuTarget::None;
    }

    if s.ends_with('%') {
        let num_str = &s[..s.len() - 1];
        if let Ok(percent) = num_str.parse::<f64>() {
            if percent > 0.0 && percent <= 100.0 {
                return CpuTarget::Dynamic(percent);
            }
        }
    }

    if let Ok(fixed) = s.parse::<u32>() {
        if fixed > 0 {
            return CpuTarget::Fixed(fixed);
        }
    }

    CpuTarget::None
}

fn parse_memory(s: &str) -> MemoryTarget {
    let s = s.trim().to_uppercase();
    if s.is_empty() || s == "0" {
        return MemoryTarget::None;
    }

    if s.ends_with('%') {
        let num_str = &s[..s.len() - 1];
        if let Ok(percent) = num_str.parse::<f64>() {
            if percent > 0.0 && percent <= 100.0 {
                return MemoryTarget::Dynamic(percent);
            }
        }
    }

    let (num_str, mult) = if let Some(num) = s.strip_suffix("GB") {
        (num, 1024 * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix('G') {
        (num, 1024 * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix("MB") {
        (num, 1024 * 1024)
    } else if let Some(num) = s.strip_suffix('M') {
        (num, 1024 * 1024)
    } else if let Some(num) = s.strip_suffix("KB") {
        (num, 1024)
    } else if let Some(num) = s.strip_suffix('K') {
        (num, 1024)
    } else {
        (s.as_str(), 1)
    };

    MemoryTarget::Fixed(num_str.parse::<usize>().unwrap_or(0) * mult)
}

fn get_total_memory() -> usize {
    sys_info::mem_info()
        .map(|m| m.total as usize * 1024)
        .unwrap_or(16 * 1024 * 1024 * 1024)
}

fn get_available_memory() -> usize {
    sys_info::mem_info()
        .map(|m| m.avail as usize * 1024)
        .unwrap_or(8 * 1024 * 1024 * 1024)
}

fn get_cpu_cores() -> u32 {
    sys_info::cpu_num().unwrap_or(1) as u32
}

fn main() {
    let args = Args::parse();
    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);

    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    let cpu_target = parse_cpu(&args.cpu);
    let memory_target = parse_memory(&args.memory);

    let cpu_cores = get_cpu_cores();
    let total_memory = get_total_memory();

    println!(
        "System: {} CPU cores, {} GB memory",
        cpu_cores,
        total_memory / (1024 * 1024 * 1024)
    );
    println!("Press Ctrl-C to stop");
    println!();

    let cpu_target_arc = match &cpu_target {
        CpuTarget::Fixed(percent) => {
            println!("CPU: fixed {}%", percent);
            Some(cpu::consume(*percent, Arc::clone(&running)))
        }
        CpuTarget::Dynamic(percent) => {
            println!("CPU: dynamic {}%", percent);
            let target = (*percent * cpu_cores as f64).min(cpu_cores as f64 * 8.0) as u32;
            Some(cpu::consume(target, Arc::clone(&running)))
        }
        CpuTarget::None => {
            println!("CPU: none");
            None
        }
    };

    let mut memory_consumer = memory::MemoryConsumer::new();

    match &memory_target {
        MemoryTarget::Fixed(bytes) => {
            println!(
                "Memory: fixed {} bytes ({:.2} GB)",
                bytes,
                *bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            );
            memory_consumer.consume(*bytes, Arc::clone(&running));
        }
        MemoryTarget::Dynamic(percent) => {
            println!("Memory: dynamic {}%", percent);
            let target_bytes = (total_memory as f64 * percent / 100.0) as usize;
            println!(
                "  Initial target: {} bytes ({:.2} GB)",
                target_bytes,
                target_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            );
            memory_consumer.consume(target_bytes, Arc::clone(&running));
        }
        MemoryTarget::None => {
            println!("Memory: none");
        }
    }

    let start = std::time::Instant::now();
    let timeout = if args.duration > 0 {
        Some(std::time::Duration::from_secs(args.duration))
    } else {
        None
    };

    let mut last_adjust = std::time::Instant::now();

    while running.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));

        if let Some(t) = timeout {
            if start.elapsed() >= t {
                println!("\nDuration timeout");
                running.store(false, Ordering::Relaxed);
            }
        }

        if last_adjust.elapsed() >= std::time::Duration::from_secs(ADJUST_INTERVAL_SECS) {
            last_adjust = std::time::Instant::now();

            let total_memory = get_total_memory();
            let available_memory = get_available_memory();
            let current_mem_usage = memory_consumer.get_current_usage();

            if let Some(ref cpu_arc) = &cpu_target_arc {
                match &cpu_target {
                    CpuTarget::Dynamic(percent) => {
                        let target_percent =
                            (*percent * cpu_cores as f64).min(cpu_cores as f64 * 8.0) as u32;
                        cpu_arc.store(target_percent, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }

            match &memory_target {
                MemoryTarget::Dynamic(percent) => {
                    let target_bytes = (total_memory as f64 * percent / 100.0) as usize;
                    if current_mem_usage != target_bytes {
                        println!(
                            "Adjusting memory: {} MB -> {} MB ({}% of {} GB)",
                            current_mem_usage / (1024 * 1024),
                            target_bytes / (1024 * 1024),
                            percent,
                            total_memory / (1024 * 1024 * 1024)
                        );
                        memory_consumer.adjust_to(target_bytes, Arc::clone(&running));
                    }
                }
                MemoryTarget::Fixed(target_bytes) => {
                    if available_memory < total_memory / 10 {
                        println!("Warning: Low memory, releasing 20%");
                        memory_consumer.release_percent(20, Arc::clone(&running));
                    } else if current_mem_usage < *target_bytes {
                        let deficit = target_bytes - current_mem_usage;
                        let add_amount = (deficit / 10).min(available_memory / 2);
                        memory_consumer
                            .adjust_to(current_mem_usage + add_amount, Arc::clone(&running));
                    }
                }
                _ => {}
            }
        }
    }

    drop(memory_consumer);
    println!("\nStopped.");
}
