use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct MemoryConsumer {
    data: Vec<Vec<u8>>,
    touch_chunk_idx: usize,
    touch_offset: usize,
}

impl MemoryConsumer {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            touch_chunk_idx: 0,
            touch_offset: 0,
        }
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
                let mut chunk = Vec::with_capacity(chunk_size);
                chunk.resize(chunk_size, 0xAA);
                self.data.push(chunk);
            }

            if remainder > 0 && running.load(Ordering::Relaxed) {
                let mut chunk = Vec::with_capacity(remainder);
                chunk.resize(remainder, 0xAA);
                self.data.push(chunk);
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

    pub fn touch_pages(&mut self, pages_to_touch: usize) {
        if pages_to_touch == 0 || self.data.is_empty() {
            return;
        }

        const PAGE_SIZE: usize = 4096;

        for _ in 0..pages_to_touch {
            if self.data.is_empty() {
                self.touch_chunk_idx = 0;
                self.touch_offset = 0;
                return;
            }

            if self.touch_chunk_idx >= self.data.len() {
                self.touch_chunk_idx = 0;
                self.touch_offset = 0;
            }

            let chunk_len = self.data[self.touch_chunk_idx].len();
            if chunk_len == 0 {
                self.touch_chunk_idx = (self.touch_chunk_idx + 1) % self.data.len();
                self.touch_offset = 0;
                continue;
            }

            if self.touch_offset >= chunk_len {
                self.touch_chunk_idx = (self.touch_chunk_idx + 1) % self.data.len();
                self.touch_offset = 0;
                continue;
            }

            let byte = &mut self.data[self.touch_chunk_idx][self.touch_offset];
            *byte = byte.wrapping_add(1);
            self.touch_offset += PAGE_SIZE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_running_flag() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(true))
    }

    #[test]
    fn test_new_consumer_has_zero_usage() {
        let consumer = MemoryConsumer::new();
        assert_eq!(consumer.get_current_usage(), 0);
    }

    #[test]
    fn test_consume_10mb() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();
        let ten_mb = 10 * 1024 * 1024;
        consumer.consume(ten_mb, running);
        assert_eq!(consumer.get_current_usage(), ten_mb);
    }

    #[test]
    fn test_consume_zero() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();
        consumer.consume(0, running);
        assert_eq!(consumer.get_current_usage(), 0);
    }

    #[test]
    fn test_consume_one_byte() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();
        consumer.consume(1, running);
        assert_eq!(consumer.get_current_usage(), 1);
    }

    #[test]
    fn test_consume_partial_mb() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();
        let partial = 512 * 1024 + 100;
        consumer.consume(partial, running);
        assert_eq!(consumer.get_current_usage(), partial);
    }

    #[test]
    fn test_adjust_to_increase() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();

        consumer.consume(5 * 1024 * 1024, Arc::clone(&running));
        assert_eq!(consumer.get_current_usage(), 5 * 1024 * 1024);

        consumer.adjust_to(10 * 1024 * 1024, running);
        assert_eq!(consumer.get_current_usage(), 10 * 1024 * 1024);
    }

    #[test]
    fn test_adjust_to_decrease() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();

        consumer.consume(10 * 1024 * 1024, Arc::clone(&running));
        assert_eq!(consumer.get_current_usage(), 10 * 1024 * 1024);

        consumer.adjust_to(5 * 1024 * 1024, running);
        assert_eq!(consumer.get_current_usage(), 5 * 1024 * 1024);
    }

    #[test]
    fn test_adjust_to_same() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();

        consumer.consume(10 * 1024 * 1024, Arc::clone(&running));
        consumer.adjust_to(10 * 1024 * 1024, running);
        assert_eq!(consumer.get_current_usage(), 10 * 1024 * 1024);
    }

    #[test]
    fn test_adjust_to_zero() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();

        consumer.consume(10 * 1024 * 1024, Arc::clone(&running));
        consumer.adjust_to(0, running);
        assert_eq!(consumer.get_current_usage(), 0);
    }

    #[test]
    fn test_release_percent_20() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();

        let initial = 100 * 1024 * 1024;
        consumer.consume(initial, Arc::clone(&running));
        assert_eq!(consumer.get_current_usage(), initial);

        consumer.release_percent(20, running);
        assert_eq!(consumer.get_current_usage(), 80 * 1024 * 1024);
    }

    #[test]
    fn test_release_percent_50() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();

        let initial = 100 * 1024 * 1024;
        consumer.consume(initial, Arc::clone(&running));

        consumer.release_percent(50, running);
        assert_eq!(consumer.get_current_usage(), 50 * 1024 * 1024);
    }

    #[test]
    fn test_release_percent_100() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();

        consumer.consume(100 * 1024 * 1024, Arc::clone(&running));
        consumer.release_percent(100, running);
        assert_eq!(consumer.get_current_usage(), 0);
    }

    #[test]
    fn test_multiple_adjustments() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();

        consumer.consume(10 * 1024 * 1024, Arc::clone(&running));
        assert_eq!(consumer.get_current_usage(), 10 * 1024 * 1024);

        consumer.adjust_to(20 * 1024 * 1024, Arc::clone(&running));
        assert_eq!(consumer.get_current_usage(), 20 * 1024 * 1024);

        consumer.adjust_to(5 * 1024 * 1024, Arc::clone(&running));
        assert_eq!(consumer.get_current_usage(), 5 * 1024 * 1024);

        consumer.adjust_to(15 * 1024 * 1024, running);
        assert_eq!(consumer.get_current_usage(), 15 * 1024 * 1024);
    }

    #[test]
    fn test_large_allocation() {
        let mut consumer = MemoryConsumer::new();
        let running = create_running_flag();

        let large = 100 * 1024 * 1024;
        consumer.consume(large, running);
        assert_eq!(consumer.get_current_usage(), large);
    }

    #[test]
    fn test_stops_on_running_false() {
        let mut consumer = MemoryConsumer::new();
        let running = Arc::new(AtomicBool::new(false));

        consumer.consume(10 * 1024 * 1024, running);
        assert_eq!(consumer.get_current_usage(), 0);
    }
}
