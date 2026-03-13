use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const CHUNK_SIZE: usize = 1024 * 1024;
const MIN_STEP: usize = 16 * 1024 * 1024;
const MAX_BOOST_FACTOR: f64 = 1.12;

pub struct MemoryController {
    target_bytes: usize,
    chunks: Vec<Vec<u8>>,
    touch_chunk_idx: usize,
    touch_offset: usize,
    max_allocated_bytes: usize,
}

impl MemoryController {
    pub fn new(target_bytes: usize) -> Self {
        Self {
            target_bytes,
            chunks: Vec::new(),
            touch_chunk_idx: 0,
            touch_offset: 0,
            max_allocated_bytes: target_bytes.max(MIN_STEP),
        }
    }

    pub fn set_target(&mut self, target_bytes: usize) {
        self.target_bytes = target_bytes;
        self.max_allocated_bytes = target_bytes.max(MIN_STEP);
    }

    pub fn allocated_bytes(&self) -> usize {
        self.chunks.iter().map(|c| c.len()).sum()
    }

    pub fn step(&mut self, observed_private_bytes: usize, running: Arc<AtomicBool>) {
        let target = self.target_bytes;
        if target == 0 {
            self.adjust_to(0, running);
            return;
        }

        let low = (target as f64 * 0.97) as usize;
        let high = (target as f64 * 1.03) as usize;
        let alloc_ceiling = (target as f64 * MAX_BOOST_FACTOR) as usize;
        let current_alloc = self.allocated_bytes();

        if observed_private_bytes < low {
            let gap = target.saturating_sub(observed_private_bytes);
            let add = (gap / 2).max(MIN_STEP).min(target / 10 + MIN_STEP);
            self.max_allocated_bytes = self
                .max_allocated_bytes
                .max(current_alloc)
                .saturating_add(add)
                .min(alloc_ceiling);
            let new_target = self.max_allocated_bytes;
            self.adjust_to(new_target, running);
        } else if observed_private_bytes > high {
            let release_to = (target as f64 * 1.01) as usize;
            self.max_allocated_bytes = target;
            self.adjust_to(release_to, running);
        } else {
            self.max_allocated_bytes = self.max_allocated_bytes.min(target);
            if current_alloc > target {
                self.adjust_to(target, running);
            }
        }
    }

    pub fn touch_hot_pages(&mut self) {
        if self.chunks.is_empty() {
            return;
        }

        let total_bytes = self.allocated_bytes();
        let pages_to_touch = ((total_bytes / 4096) / 4).clamp(256, 131072);
        const PAGE_SIZE: usize = 4096;

        for _ in 0..pages_to_touch {
            if self.chunks.is_empty() {
                self.touch_chunk_idx = 0;
                self.touch_offset = 0;
                return;
            }

            if self.touch_chunk_idx >= self.chunks.len() {
                self.touch_chunk_idx = 0;
                self.touch_offset = 0;
            }

            let len = self.chunks[self.touch_chunk_idx].len();
            if len == 0 {
                self.touch_chunk_idx = (self.touch_chunk_idx + 1) % self.chunks.len();
                self.touch_offset = 0;
                continue;
            }

            if self.touch_offset >= len {
                self.touch_chunk_idx = (self.touch_chunk_idx + 1) % self.chunks.len();
                self.touch_offset = 0;
                continue;
            }

            let b = &mut self.chunks[self.touch_chunk_idx][self.touch_offset];
            *b = b.wrapping_add(1);
            self.touch_offset += PAGE_SIZE;
        }
    }

    fn adjust_to(&mut self, target_bytes: usize, running: Arc<AtomicBool>) {
        let mut current = self.allocated_bytes();

        while current < target_bytes && running.load(Ordering::Relaxed) {
            let remaining = target_bytes - current;
            let this_chunk = remaining.min(CHUNK_SIZE);

            let mut chunk = Vec::with_capacity(this_chunk);
            chunk.resize(this_chunk, 0xAA);
            self.chunks.push(chunk);
            current += this_chunk;
        }

        while current > target_bytes && !self.chunks.is_empty() && running.load(Ordering::Relaxed) {
            if let Some(chunk) = self.chunks.pop() {
                current = current.saturating_sub(chunk.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(true))
    }

    #[test]
    fn test_zero_target_releases_all() {
        let mut c = MemoryController::new(50 * 1024 * 1024);
        c.adjust_to(50 * 1024 * 1024, running());
        assert!(c.allocated_bytes() > 0);
        c.set_target(0);
        c.step(0, running());
        assert_eq!(c.allocated_bytes(), 0);
    }

    #[test]
    fn test_step_adds_memory_when_below_low() {
        let mut c = MemoryController::new(100 * 1024 * 1024);
        c.step(0, running());
        assert!(c.allocated_bytes() >= 16 * 1024 * 1024);
    }

    #[test]
    fn test_step_can_overallocate_to_chase_observed_private() {
        let mut c = MemoryController::new(100 * 1024 * 1024);
        c.step(0, running());
        let first = c.allocated_bytes();
        c.step(0, running());
        assert!(c.allocated_bytes() >= first);
    }

    #[test]
    fn test_step_caps_overallocation() {
        let target = 100 * 1024 * 1024;
        let mut c = MemoryController::new(target);
        for _ in 0..20 {
            c.step(0, running());
        }
        assert!(c.allocated_bytes() <= (target as f64 * MAX_BOOST_FACTOR) as usize);
    }
}
