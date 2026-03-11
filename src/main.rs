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

    if let Some(num_str) = s.strip_suffix('%') {
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

    if let Some(num_str) = s.strip_suffix('%') {
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
            let target = (*percent * cpu_cores as f64).min(cpu_cores as f64 * 100.0) as u32;
            let arc = cpu::consume(target, Arc::clone(&running));
            Some(arc)
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

            let cpu_percent = cpu_target_arc
                .as_ref()
                .map(|arc| arc.load(Ordering::Relaxed))
                .unwrap_or(0);
            let mem_gb = current_mem_usage as f64 / (1024.0 * 1024.0 * 1024.0);
            let total_mem_gb = total_memory as f64 / (1024.0 * 1024.0 * 1024.0);
            let mem_percent = if total_memory > 0 {
                let pct = current_mem_usage as f64 / total_memory as f64 * 100.0;
                pct.round() as u32
            } else {
                0
            };

            println!(
                "[Running] CPU: {}% | MEM: {:.2} GB / {:.2} GB ({}%)",
                cpu_percent, mem_gb, total_mem_gb, mem_percent
            );

            if let Some(ref cpu_arc) = &cpu_target_arc {
                if let CpuTarget::Dynamic(percent) = &cpu_target {
                    let target_percent =
                        (*percent * cpu_cores as f64).min(cpu_cores as f64 * 100.0) as u32;
                    cpu_arc.store(target_percent, Ordering::Relaxed);
                }
            }

            match &memory_target {
                MemoryTarget::Dynamic(percent) => {
                    let target_bytes = (total_memory as f64 * percent / 100.0) as usize;
                    if current_mem_usage != target_bytes {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_fixed() {
        assert!(matches!(parse_cpu("100"), CpuTarget::Fixed(100)));
        assert!(matches!(parse_cpu("200"), CpuTarget::Fixed(200)));
        assert!(matches!(parse_cpu("1"), CpuTarget::Fixed(1)));
    }

    #[test]
    fn test_parse_cpu_dynamic() {
        match parse_cpu("50%") {
            CpuTarget::Dynamic(p) => assert!((p - 50.0).abs() < 0.01),
            _ => panic!("Expected Dynamic"),
        }
        match parse_cpu("100%") {
            CpuTarget::Dynamic(p) => assert!((p - 100.0).abs() < 0.01),
            _ => panic!("Expected Dynamic"),
        }
        match parse_cpu("25.5%") {
            CpuTarget::Dynamic(p) => assert!((p - 25.5).abs() < 0.01),
            _ => panic!("Expected Dynamic"),
        }
    }

    #[test]
    fn test_parse_cpu_none() {
        assert!(matches!(parse_cpu("0"), CpuTarget::None));
        assert!(matches!(parse_cpu(""), CpuTarget::None));
        assert!(matches!(parse_cpu("   "), CpuTarget::None));
        assert!(matches!(parse_cpu("invalid"), CpuTarget::None));
        assert!(matches!(parse_cpu("abc%"), CpuTarget::None));
    }

    #[test]
    fn test_parse_cpu_trim() {
        match parse_cpu("  50%  ") {
            CpuTarget::Dynamic(p) => assert!((p - 50.0).abs() < 0.01),
            _ => panic!("Expected Dynamic"),
        }
    }

    #[test]
    fn test_parse_memory_gb() {
        match parse_memory("2GB") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 2 * 1024 * 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
        match parse_memory("2G") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 2 * 1024 * 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
        match parse_memory("1GB") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 1024 * 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
    }

    #[test]
    fn test_parse_memory_mb() {
        match parse_memory("512M") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 512 * 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
        match parse_memory("512MB") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 512 * 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
    }

    #[test]
    fn test_parse_memory_kb() {
        match parse_memory("1024K") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
        match parse_memory("1024KB") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
    }

    #[test]
    fn test_parse_memory_bytes() {
        match parse_memory("1024") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 1024),
            _ => panic!("Expected Fixed"),
        }
    }

    #[test]
    fn test_parse_memory_dynamic() {
        match parse_memory("50%") {
            MemoryTarget::Dynamic(p) => assert!((p - 50.0).abs() < 0.01),
            _ => panic!("Expected Dynamic"),
        }
        match parse_memory("100%") {
            MemoryTarget::Dynamic(p) => assert!((p - 100.0).abs() < 0.01),
            _ => panic!("Expected Dynamic"),
        }
    }

    #[test]
    fn test_parse_memory_none() {
        assert!(matches!(parse_memory("0"), MemoryTarget::None));
        assert!(matches!(parse_memory(""), MemoryTarget::None));
        assert!(matches!(parse_memory("   "), MemoryTarget::None));
    }

    #[test]
    fn test_parse_memory_case_insensitive() {
        match parse_memory("2gb") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 2 * 1024 * 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
        match parse_memory("2Gb") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 2 * 1024 * 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
    }

    #[test]
    fn test_parse_memory_trim() {
        match parse_memory("  2GB  ") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 2 * 1024 * 1024 * 1024),
            _ => panic!("Expected Fixed"),
        }
    }

    #[test]
    fn test_parse_memory_invalid() {
        match parse_memory("abc") {
            MemoryTarget::Fixed(bytes) => assert_eq!(bytes, 0),
            _ => panic!("Expected Fixed with 0"),
        }
    }
}
