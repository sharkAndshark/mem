use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct CpuConsumer {
    target_percent: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
}

impl CpuConsumer {
    pub fn new(initial_percent: u32, running: Arc<AtomicBool>) -> Self {
        Self {
            target_percent: Arc::new(AtomicU32::new(initial_percent)),
            running,
        }
    }

    pub fn start(&self) {
        let max_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(1);
        for thread_id in 0..max_threads {
            let target = Arc::clone(&self.target_percent);
            let running = Arc::clone(&self.running);

            thread::spawn(move || {
                Self::worker_loop(thread_id, target, running);
            });
        }
    }

    fn worker_loop(thread_id: usize, target: Arc<AtomicU32>, running: Arc<AtomicBool>) {
        let cycle_ms = 50;
        loop {
            if !running.load(Ordering::Relaxed) {
                break;
            }

            let current_target = target.load(Ordering::Relaxed) as f64;

            if current_target <= 0.0 {
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            let max_threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                .max(1);
            let total_threads = ((current_target / 100.0).ceil() as usize).clamp(1, max_threads);

            if thread_id >= total_threads {
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            let percent_per_thread = current_target / total_threads as f64;
            let work_ratio = (percent_per_thread / 100.0).clamp(0.0, 1.0);

            let work_micros = (cycle_ms as f64 * work_ratio * 1000.0) as u64;
            let work_duration = Duration::from_micros(work_micros);
            let sleep_duration = Duration::from_millis(cycle_ms).saturating_sub(work_duration);

            let busy_start = std::time::Instant::now();
            let mut x: u64 = 0;
            while busy_start.elapsed() < work_duration && running.load(Ordering::Relaxed) {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                std::hint::black_box(x);
            }

            if !running.load(Ordering::Relaxed) {
                break;
            }

            thread::sleep(sleep_duration);
        }
    }
}

pub fn consume(target_percent: u32, running: Arc<AtomicBool>) -> Arc<AtomicU32> {
    let consumer = CpuConsumer::new(target_percent, running);
    consumer.start();
    Arc::clone(&consumer.target_percent)
}
