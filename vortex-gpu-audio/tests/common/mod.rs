/// Common test utilities and fixtures for integration tests
/// 
/// This module provides shared functionality for all integration tests.

use std::path::PathBuf;

/// Test configuration for audio processing
pub struct TestConfig {
    pub sample_rate: u32,
    pub buffer_size: usize,
    pub channels: u16,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            buffer_size: 512,
            channels: 2,
        }
    }
}

/// Generate a sine wave for testing
/// 
/// # Arguments
/// * `frequency` - Frequency in Hz
/// * `duration_secs` - Duration in seconds
/// * `sample_rate` - Sample rate in Hz
/// 
/// # Returns
/// Vector of f32 samples
pub fn generate_sine_wave(frequency: f32, duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    let mut samples = Vec::with_capacity(num_samples);
    
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (2.0 * std::f32::consts::PI * frequency * t).sin();
        samples.push(sample);
    }
    
    samples
}

/// Generate white noise for testing
pub fn generate_white_noise(duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    use std::cell::RefCell;
    
    // Simple LCG random number generator for deterministic tests
    thread_local! {
        static SEED: RefCell<u32> = RefCell::new(12345);
    }
    
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    let mut samples = Vec::with_capacity(num_samples);
    
    for _ in 0..num_samples {
        SEED.with(|seed| {
            let mut s = seed.borrow_mut();
            *s = s.wrapping_mul(1103515245).wrapping_add(12345);
            let normalized = (*s as f32 / u32::MAX as f32) * 2.0 - 1.0;
            samples.push(normalized);
        });
    }
    
    samples
}

/// Generate digital silence
pub fn generate_silence(duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    vec![0.0; num_samples]
}

/// Generate an impulse signal (single sample at 1.0)
pub fn generate_impulse(sample_rate: u32) -> Vec<f32> {
    let mut samples = vec![0.0; sample_rate as usize];
    samples[0] = 1.0;
    samples
}

/// Calculate RMS (Root Mean Square) of a signal
pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    
    let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_squares / samples.len() as f32).sqrt()
}

/// Calculate peak amplitude
pub fn calculate_peak(samples: &[f32]) -> f32 {
    samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max)
}

/// Compare two audio buffers with tolerance
pub fn buffers_are_similar(a: &[f32], b: &[f32], tolerance: f32) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    for (sample_a, sample_b) in a.iter().zip(b.iter()) {
        if (sample_a - sample_b).abs() > tolerance {
            return false;
        }
    }
    
    true
}

/// Create a temporary test directory
pub fn create_temp_test_dir() -> PathBuf {
    let temp_dir = std::env::temp_dir().join(format!("vortex_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    temp_dir
}

/// Clean up temporary test directory
pub fn cleanup_test_dir(path: &PathBuf) {
    let _ = std::fs::remove_dir_all(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_wave_generation() {
        let samples = generate_sine_wave(1000.0, 0.1, 48000);
        assert_eq!(samples.len(), 4800);
        
        // Check that signal oscillates
        let peak = calculate_peak(&samples);
        assert!(peak > 0.9 && peak < 1.1);
    }

    #[test]
    fn test_white_noise_generation() {
        let samples = generate_white_noise(0.1, 48000);
        assert_eq!(samples.len(), 4800);
        
        // Noise should have reasonable amplitude
        let rms = calculate_rms(&samples);
        assert!(rms > 0.3 && rms < 0.7);
    }

    #[test]
    fn test_silence_generation() {
        let samples = generate_silence(0.1, 48000);
        assert_eq!(samples.len(), 4800);
        assert_eq!(calculate_rms(&samples), 0.0);
    }

    #[test]
    fn test_impulse_generation() {
        let samples = generate_impulse(48000);
        assert_eq!(samples.len(), 48000);
        assert_eq!(samples[0], 1.0);
        assert_eq!(calculate_rms(&samples[1..]), 0.0);
    }

    #[test]
    fn test_rms_calculation() {
        let samples = vec![1.0, -1.0, 1.0, -1.0];
        let rms = calculate_rms(&samples);
        assert!((rms - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_peak_calculation() {
        let samples = vec![0.5, -0.8, 0.3, -0.9];
        let peak = calculate_peak(&samples);
        assert_eq!(peak, 0.9);
    }

    #[test]
    fn test_buffer_similarity() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.01, 2.01, 3.01];
        assert!(buffers_are_similar(&a, &b, 0.02));
        assert!(!buffers_are_similar(&a, &b, 0.005));
    }
}
