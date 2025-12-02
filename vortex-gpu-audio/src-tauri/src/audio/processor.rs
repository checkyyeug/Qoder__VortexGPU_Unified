use crate::error::{AudioError, VortexError};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Processing statistics
#[derive(Debug, Clone)]
pub struct ProcessingStats {
    pub samples_processed: u64,
    pub buffer_underruns: usize,
    pub buffer_overruns: usize,
    pub average_latency_us: f64,
    pub peak_latency_us: u64,
    pub cpu_usage_percent: f32,
}

impl Default for ProcessingStats {
    fn default() -> Self {
        Self {
            samples_processed: 0,
            buffer_underruns: 0,
            buffer_overruns: 0,
            average_latency_us: 0.0,
            peak_latency_us: 0,
            cpu_usage_percent: 0.0,
        }
    }
}

/// Real-time audio processor
pub struct AudioProcessor {
    sample_rate: u32,
    buffer_size: usize,
    channels: u16,
    
    // Statistics
    samples_processed: AtomicU64,
    underruns: AtomicUsize,
    overruns: AtomicUsize,
    
    // Latency tracking
    latency_sum_us: AtomicU64,
    latency_count: AtomicU64,
    peak_latency_us: AtomicU64,
    
    // Timing
    last_process_time: parking_lot::Mutex<Option<Instant>>,
}

impl AudioProcessor {
    /// Create a new audio processor
    pub fn new(
        sample_rate: u32,
        buffer_size: usize,
        channels: u16,
    ) -> Result<Self, VortexError> {
        if sample_rate == 0 {
            return Err(AudioError::InvalidParameter(
                "Sample rate must be > 0".to_string()
            ).into());
        }
        
        if buffer_size == 0 || !buffer_size.is_power_of_two() {
            return Err(AudioError::InvalidParameter(
                "Buffer size must be power of 2".to_string()
            ).into());
        }
        
        if channels == 0 {
            return Err(AudioError::InvalidParameter(
                "Channel count must be > 0".to_string()
            ).into());
        }
        
        Ok(Self {
            sample_rate,
            buffer_size,
            channels,
            samples_processed: AtomicU64::new(0),
            underruns: AtomicUsize::new(0),
            overruns: AtomicUsize::new(0),
            latency_sum_us: AtomicU64::new(0),
            latency_count: AtomicU64::new(0),
            peak_latency_us: AtomicU64::new(0),
            last_process_time: parking_lot::Mutex::new(None),
        })
    }
    
    /// Update processing statistics
    pub fn update_stats(&self, samples: usize) {
        self.samples_processed.fetch_add(samples as u64, Ordering::Relaxed);
    }
    
    /// Record a buffer underrun
    pub fn record_underrun(&self) {
        self.underruns.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a buffer overrun
    pub fn record_overrun(&self) {
        self.overruns.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record processing latency
    pub fn record_latency(&self, duration: Duration) {
        let latency_us = duration.as_micros() as u64;
        
        self.latency_sum_us.fetch_add(latency_us, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
        
        // Update peak latency
        let mut peak = self.peak_latency_us.load(Ordering::Relaxed);
        while latency_us > peak {
            match self.peak_latency_us.compare_exchange_weak(
                peak,
                latency_us,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => peak = current,
            }
        }
    }
    
    /// Get current statistics
    pub fn get_stats(&self) -> ProcessingStats {
        let samples = self.samples_processed.load(Ordering::Relaxed);
        let underruns = self.underruns.load(Ordering::Relaxed);
        let overruns = self.overruns.load(Ordering::Relaxed);
        let latency_sum = self.latency_sum_us.load(Ordering::Relaxed);
        let latency_count = self.latency_count.load(Ordering::Relaxed);
        let peak = self.peak_latency_us.load(Ordering::Relaxed);
        
        let average_latency = if latency_count > 0 {
            latency_sum as f64 / latency_count as f64
        } else {
            0.0
        };
        
        // Calculate CPU usage based on buffer size and sample rate
        let buffer_duration_us = (self.buffer_size as f64 / self.sample_rate as f64) * 1_000_000.0;
        let cpu_usage = if buffer_duration_us > 0.0 {
            ((average_latency / buffer_duration_us) * 100.0) as f32
        } else {
            0.0
        };
        
        ProcessingStats {
            samples_processed: samples,
            buffer_underruns: underruns,
            buffer_overruns: overruns,
            average_latency_us: average_latency,
            peak_latency_us: peak,
            cpu_usage_percent: cpu_usage.min(100.0),
        }
    }
    
    /// Reset statistics
    pub fn reset_stats(&self) {
        self.samples_processed.store(0, Ordering::Relaxed);
        self.underruns.store(0, Ordering::Relaxed);
        self.overruns.store(0, Ordering::Relaxed);
        self.latency_sum_us.store(0, Ordering::Relaxed);
        self.latency_count.store(0, Ordering::Relaxed);
        self.peak_latency_us.store(0, Ordering::Relaxed);
    }
    
    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    
    /// Get buffer size
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
    
    /// Get channel count
    pub fn channels(&self) -> u16 {
        self.channels
    }
    
    /// Calculate expected buffer duration in microseconds
    pub fn buffer_duration_us(&self) -> f64 {
        (self.buffer_size as f64 / self.sample_rate as f64) * 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_processor_creation() {
        let processor = AudioProcessor::new(48000, 512, 2);
        assert!(processor.is_ok());
    }
    
    #[test]
    fn test_invalid_sample_rate() {
        let processor = AudioProcessor::new(0, 512, 2);
        assert!(processor.is_err());
    }
    
    #[test]
    fn test_invalid_buffer_size() {
        // Non-power-of-2
        let processor = AudioProcessor::new(48000, 500, 2);
        assert!(processor.is_err());
        
        // Zero
        let processor = AudioProcessor::new(48000, 0, 2);
        assert!(processor.is_err());
    }
    
    #[test]
    fn test_invalid_channels() {
        let processor = AudioProcessor::new(48000, 512, 0);
        assert!(processor.is_err());
    }
    
    #[test]
    fn test_stats_update() {
        let processor = AudioProcessor::new(48000, 512, 2).unwrap();
        
        processor.update_stats(1024);
        let stats = processor.get_stats();
        assert_eq!(stats.samples_processed, 1024);
    }
    
    #[test]
    fn test_underrun_tracking() {
        let processor = AudioProcessor::new(48000, 512, 2).unwrap();
        
        processor.record_underrun();
        processor.record_underrun();
        
        let stats = processor.get_stats();
        assert_eq!(stats.buffer_underruns, 2);
    }
    
    #[test]
    fn test_latency_tracking() {
        let processor = AudioProcessor::new(48000, 512, 2).unwrap();
        
        processor.record_latency(Duration::from_micros(1000));
        processor.record_latency(Duration::from_micros(2000));
        processor.record_latency(Duration::from_micros(1500));
        
        let stats = processor.get_stats();
        assert_eq!(stats.peak_latency_us, 2000);
        assert!((stats.average_latency_us - 1500.0).abs() < 1.0);
    }
    
    #[test]
    fn test_buffer_duration() {
        let processor = AudioProcessor::new(48000, 512, 2).unwrap();
        let duration = processor.buffer_duration_us();
        
        // 512 samples at 48kHz = ~10667 microseconds
        assert!((duration - 10666.67).abs() < 1.0);
    }
    
    #[test]
    fn test_stats_reset() {
        let processor = AudioProcessor::new(48000, 512, 2).unwrap();
        
        processor.update_stats(1024);
        processor.record_underrun();
        processor.record_latency(Duration::from_micros(1000));
        
        processor.reset_stats();
        
        let stats = processor.get_stats();
        assert_eq!(stats.samples_processed, 0);
        assert_eq!(stats.buffer_underruns, 0);
        assert_eq!(stats.peak_latency_us, 0);
    }
    
    #[test]
    fn test_cpu_usage_calculation() {
        let processor = AudioProcessor::new(48000, 512, 2).unwrap();
        let buffer_duration = processor.buffer_duration_us();
        
        // Simulate processing taking half the buffer duration
        let processing_time = Duration::from_micros((buffer_duration / 2.0) as u64);
        processor.record_latency(processing_time);
        
        let stats = processor.get_stats();
        // CPU usage should be around 50%
        assert!((stats.cpu_usage_percent - 50.0).abs() < 1.0);
    }
}
