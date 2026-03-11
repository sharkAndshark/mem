use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct MemoryConsumer {
    data: Vec<Vec<u8>>,
}

impl MemoryConsumer {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn get_current_usage(&self) -> usize {
        self.data.iter().map(|chunk| chunk.len()).sum()
    }

    pub fn consume(&mut self, bytes: usize, running: Arc<AtomicBool>) {
        self.adjust_to(bytes, running);
    }

    pub fn adjust_to(&mut self, target_bytes: usize, running: Arc<AtomicBool>) {
        let current = self.get_current_usage();

        if target_bytes > current {
            let to_add = target_bytes - current;
            let chunk_size = 1024 * 1024;
            let chunks = to_add / chunk_size;
            let remainder = to_add % chunk_size;

            for _ in 0..chunks {
                if !running.load(Ordering::Relaxed) {
                    return;
                }
                self.data.push(vec![0xAAu8; chunk_size]);
            }

            if remainder > 0 && running.load(Ordering::Relaxed) {
                self.data.push(vec![0xAAu8; remainder]);
            }
        } else if target_bytes < current {
            let to_release = current - target_bytes;
            let mut released = 0;

            while released < to_release && !self.data.is_empty() {
                if !running.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(chunk) = self.data.pop() {
                    released += chunk.len();
                }
            }
        }
    }

    pub fn release_percent(&mut self, percent: u32, running: Arc<AtomicBool>) {
        let current = self.get_current_usage();
        let to_release = current * percent as usize / 100;
        let new_target = current.saturating_sub(to_release);
        self.adjust_to(new_target, running);
    }
}
