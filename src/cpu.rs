use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const CYCLE_MS: u64 = 50;

pub struct CpuController {
    duty_percent: Arc<AtomicU32>,
    target_percent: u32,
    running: Arc<AtomicBool>,
}

impl CpuController {
    pub fn new(target_percent: u32, running: Arc<AtomicBool>) -> Self {
        Self {
            duty_percent: Arc::new(AtomicU32::new(target_percent.min(100))),
            target_percent: target_percent.min(100),
            running,
        }
    }

    pub fn set_target_percent(&self, percent: u32) {
        self.duty_percent.store(percent.min(100), Ordering::Relaxed);
    }

    pub fn update_from_observed(&self, observed_percent: f64) {
        let current = self.duty_percent.load(Ordering::Relaxed);
        let target = self.target_percent as f64;
        let next = if observed_percent + 2.0 < target {
            current.saturating_add(5)
        } else if observed_percent > target + 2.0 {
            current.saturating_sub(5)
        } else {
            current
        };
        self.duty_percent
            .store(next.clamp(0, 100), Ordering::Relaxed);
    }

    pub fn get_target_percent(&self) -> u32 {
        self.target_percent
    }

    pub fn get_duty_percent(&self) -> u32 {
        self.duty_percent.load(Ordering::Relaxed)
    }

    pub fn start(&self) {
        let worker_count = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(1);

        for worker_id in 0..worker_count {
            let running = Arc::clone(&self.running);
            let target = Arc::clone(&self.duty_percent);

            thread::spawn(move || {
                worker_loop(worker_id, worker_count, target, running);
            });
        }
    }
}

fn worker_loop(
    worker_id: usize,
    worker_count: usize,
    target_percent: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
) {
    let cycle = Duration::from_millis(CYCLE_MS);
    let cycle_micros = cycle.as_micros() as f64;

    while running.load(Ordering::Relaxed) {
        let global_target = target_percent.load(Ordering::Relaxed).min(100) as f64;
        if global_target <= 0.0 {
            thread::sleep(cycle);
            continue;
        }

        let per_worker_ratio = (global_target / 100.0).clamp(0.0, 1.0);

        if worker_id >= worker_count {
            thread::sleep(cycle);
            continue;
        }

        let work_micros = (cycle_micros * per_worker_ratio) as u64;
        let work_duration = Duration::from_micros(work_micros);
        let sleep_duration = cycle.saturating_sub(work_duration);

        let start = std::time::Instant::now();
        let mut x = 0_u64;
        while start.elapsed() < work_duration && running.load(Ordering::Relaxed) {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
            std::hint::black_box(x);
        }

        if !running.load(Ordering::Relaxed) {
            break;
        }

        thread::sleep(sleep_duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_clamped() {
        let running = Arc::new(AtomicBool::new(true));
        let c = CpuController::new(150, Arc::clone(&running));
        assert_eq!(c.get_target_percent(), 100);
        c.set_target_percent(250);
        assert_eq!(c.get_duty_percent(), 100);
    }
}
