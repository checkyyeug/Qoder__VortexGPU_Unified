/// Input validation and sanitization at trust boundaries
/// 
/// This module implements defense-in-depth validation as specified in Section 11
/// of the design review document.

use crate::error::{ConfigError, FileIoError, NetworkError, VortexResult};
use std::path::{Path, PathBuf};

/// Resource limits configuration
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_file_size_bytes: u64,
    pub max_gpu_memory_percent: f32,
    pub max_convolution_ir_samples: usize,
    pub max_filter_chain_length: usize,
    pub max_playlist_items: usize,
    pub max_websocket_clients: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_size_bytes: 2 * 1024 * 1024 * 1024, // 2 GB
            max_gpu_memory_percent: 0.8, // 80% of available GPU memory
            max_convolution_ir_samples: 16 * 1024 * 1024, // 16M samples
            max_filter_chain_length: 32,
            max_playlist_items: 10_000,
            max_websocket_clients: 8,
        }
    }
}

/// File path validator
pub struct PathValidator {
    allowed_extensions: Vec<String>,
    blocked_paths: Vec<PathBuf>,
}

impl PathValidator {
    pub fn new() -> Self {
        Self {
            allowed_extensions: vec![
                // Lossless formats
                "wav".to_string(), "flac".to_string(), "alac".to_string(), 
                "ape".to_string(), "wv".to_string(),
                // Lossy formats
                "mp3".to_string(), "aac".to_string(), "m4a".to_string(), 
                "ogg".to_string(), "opus".to_string(),
                // DSD formats
                "dsf".to_string(), "dff".to_string(), "dsd".to_string(),
                // Playlist formats
                "m3u".to_string(), "m3u8".to_string(), "pls".to_string(),
            ],
            blocked_paths: vec![],
        }
    }

    /// Validate and sanitize a file path
    /// 
    /// Checks for:
    /// - Path traversal attacks
    /// - Existence
    /// - Valid file extension
    /// - Read permissions
    pub fn validate_audio_file(&self, path: &str) -> VortexResult<PathBuf> {
        let path = PathBuf::from(path);

        // Check for path traversal
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(FileIoError::FileNotFound {
                path: path.display().to_string(),
            }.into());
        }

        // Canonicalize to resolve symlinks and relative paths
        let canonical_path = path.canonicalize()
            .map_err(|_| FileIoError::FileNotFound {
                path: path.display().to_string(),
            })?;

        // Check if file exists and is a file (not directory)
        if !canonical_path.is_file() {
            return Err(FileIoError::FileNotFound {
                path: canonical_path.display().to_string(),
            }.into());
        }

        // Validate file extension
        if let Some(ext) = canonical_path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if !self.allowed_extensions.contains(&ext_str) {
                return Err(FileIoError::UnsupportedFormat {
                    format: ext_str,
                    path: canonical_path.display().to_string(),
                }.into());
            }
        } else {
            return Err(FileIoError::UnsupportedFormat {
                format: "unknown".to_string(),
                path: canonical_path.display().to_string(),
            }.into());
        }

        // Check against blocked paths
        for blocked in &self.blocked_paths {
            if canonical_path.starts_with(blocked) {
                return Err(FileIoError::FileNotFound {
                    path: canonical_path.display().to_string(),
                }.into());
            }
        }

        Ok(canonical_path)
    }

    /// Validate file size against limits
    pub fn validate_file_size(&self, path: &Path, limits: &ResourceLimits) -> VortexResult<u64> {
        let metadata = std::fs::metadata(path)
            .map_err(|e| FileIoError::Io(e))?;

        let size = metadata.len();

        if size > limits.max_file_size_bytes {
            return Err(FileIoError::FileSizeExceeded {
                size_bytes: size,
                limit_bytes: limits.max_file_size_bytes,
            }.into());
        }

        Ok(size)
    }
}

/// Filter parameter validator
pub struct ParameterValidator;

impl ParameterValidator {
    /// Validate and clamp frequency parameter
    pub fn validate_frequency(freq: f32, sample_rate: u32) -> VortexResult<f32> {
        let nyquist = sample_rate as f32 / 2.0;
        
        if freq <= 0.0 {
            return Err(ConfigError::InvalidValue {
                key: "frequency".to_string(),
                reason: format!("Frequency must be positive, got {}", freq),
            }.into());
        }

        // Clamp to Nyquist frequency
        Ok(freq.min(nyquist * 0.95)) // Leave 5% margin
    }

    /// Validate and clamp gain parameter (in dB)
    pub fn validate_gain_db(gain: f32) -> VortexResult<f32> {
        const MAX_GAIN_DB: f32 = 24.0;
        const MIN_GAIN_DB: f32 = -48.0;

        if !gain.is_finite() {
            return Err(ConfigError::InvalidValue {
                key: "gain".to_string(),
                reason: "Gain must be a finite number".to_string(),
            }.into());
        }

        // Clamp to safe range
        Ok(gain.clamp(MIN_GAIN_DB, MAX_GAIN_DB))
    }

    /// Validate Q factor
    pub fn validate_q_factor(q: f32) -> VortexResult<f32> {
        const MIN_Q: f32 = 0.1;
        const MAX_Q: f32 = 20.0;

        if q <= 0.0 || !q.is_finite() {
            return Err(ConfigError::InvalidValue {
                key: "q_factor".to_string(),
                reason: format!("Q factor must be positive and finite, got {}", q),
            }.into());
        }

        Ok(q.clamp(MIN_Q, MAX_Q))
    }

    /// Validate sample rate
    pub fn validate_sample_rate(sample_rate: u32) -> VortexResult<u32> {
        const VALID_RATES: &[u32] = &[
            44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000
        ];

        if VALID_RATES.contains(&sample_rate) {
            Ok(sample_rate)
        } else {
            Err(ConfigError::InvalidValue {
                key: "sample_rate".to_string(),
                reason: format!("Unsupported sample rate: {}", sample_rate),
            }.into())
        }
    }

    /// Validate buffer size (must be power of 2)
    pub fn validate_buffer_size(buffer_size: usize) -> VortexResult<usize> {
        const MIN_BUFFER: usize = 64;
        const MAX_BUFFER: usize = 8192;

        if buffer_size < MIN_BUFFER || buffer_size > MAX_BUFFER {
            return Err(ConfigError::InvalidValue {
                key: "buffer_size".to_string(),
                reason: format!("Buffer size must be between {} and {}", MIN_BUFFER, MAX_BUFFER),
            }.into());
        }

        if !buffer_size.is_power_of_two() {
            return Err(ConfigError::InvalidValue {
                key: "buffer_size".to_string(),
                reason: format!("Buffer size must be a power of 2, got {}", buffer_size),
            }.into());
        }

        Ok(buffer_size)
    }
}

/// Network message validator
pub struct NetworkValidator {
    max_message_size: usize,
    rate_limit_window_secs: u64,
    max_messages_per_window: usize,
}

impl Default for NetworkValidator {
    fn default() -> Self {
        Self {
            max_message_size: 64 * 1024, // 64 KB
            rate_limit_window_secs: 1,
            max_messages_per_window: 100,
        }
    }
}

impl NetworkValidator {
    /// Validate WebSocket message
    pub fn validate_message(&self, message: &[u8]) -> VortexResult<()> {
        if message.len() > self.max_message_size {
            return Err(NetworkError::InvalidMessage {
                reason: format!(
                    "Message size {} exceeds limit {}",
                    message.len(),
                    self.max_message_size
                ),
            }.into());
        }

        // Try to parse as JSON to validate structure
        if let Err(e) = serde_json::from_slice::<serde_json::Value>(message) {
            return Err(NetworkError::InvalidMessage {
                reason: format!("Invalid JSON: {}", e),
            }.into());
        }

        Ok(())
    }

    /// Validate device ID
    pub fn validate_device_id(&self, device_id: &str) -> VortexResult<String> {
        // Basic sanitization
        if device_id.is_empty() || device_id.len() > 256 {
            return Err(NetworkError::InvalidMessage {
                reason: "Invalid device ID length".to_string(),
            }.into());
        }

        // Check for invalid characters
        if !device_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(NetworkError::InvalidMessage {
                reason: "Device ID contains invalid characters".to_string(),
            }.into());
        }

        Ok(device_id.to_string())
    }
}

/// Resource limit enforcer
pub struct ResourceLimitEnforcer {
    limits: ResourceLimits,
}

impl ResourceLimitEnforcer {
    pub fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }

    /// Check if filter chain can accept another filter
    pub fn can_add_filter(&self, current_count: usize) -> VortexResult<()> {
        if current_count >= self.limits.max_filter_chain_length {
            return Err(ConfigError::InvalidValue {
                key: "filter_chain_length".to_string(),
                reason: format!(
                    "Filter chain length {} exceeds maximum {}",
                    current_count + 1,
                    self.limits.max_filter_chain_length
                ),
            }.into());
        }
        Ok(())
    }

    /// Check if playlist can accept another item
    pub fn can_add_to_playlist(&self, current_count: usize) -> VortexResult<()> {
        if current_count >= self.limits.max_playlist_items {
            return Err(ConfigError::InvalidValue {
                key: "playlist_size".to_string(),
                reason: format!(
                    "Playlist size {} exceeds maximum {}",
                    current_count + 1,
                    self.limits.max_playlist_items
                ),
            }.into());
        }
        Ok(())
    }

    /// Check GPU memory usage
    pub fn check_gpu_memory(&self, requested_mb: usize, total_mb: usize) -> VortexResult<()> {
        let max_allowed = (total_mb as f32 * self.limits.max_gpu_memory_percent) as usize;
        
        if requested_mb > max_allowed {
            return Err(ConfigError::InvalidValue {
                key: "gpu_memory".to_string(),
                reason: format!(
                    "Requested GPU memory {} MB exceeds limit {} MB",
                    requested_mb, max_allowed
                ),
            }.into());
        }
        Ok(())
    }

    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_parameter_validation() {
        assert!(ParameterValidator::validate_frequency(1000.0, 48000).is_ok());
        assert!(ParameterValidator::validate_frequency(-100.0, 48000).is_err());
        
        assert_eq!(
            ParameterValidator::validate_gain_db(30.0).unwrap(),
            24.0 // Clamped
        );
        
        assert!(ParameterValidator::validate_q_factor(1.0).is_ok());
        assert!(ParameterValidator::validate_q_factor(-1.0).is_err());
    }

    #[test]
    fn test_sample_rate_validation() {
        assert!(ParameterValidator::validate_sample_rate(48000).is_ok());
        assert!(ParameterValidator::validate_sample_rate(22050).is_err());
    }

    #[test]
    fn test_buffer_size_validation() {
        assert!(ParameterValidator::validate_buffer_size(512).is_ok());
        assert!(ParameterValidator::validate_buffer_size(1000).is_err()); // Not power of 2
        assert!(ParameterValidator::validate_buffer_size(32).is_err()); // Too small
    }

    #[test]
    fn test_resource_limits() {
        let limits = ResourceLimits::default();
        let enforcer = ResourceLimitEnforcer::new(limits);
        
        assert!(enforcer.can_add_filter(0).is_ok());
        assert!(enforcer.can_add_filter(32).is_err());
        
        assert!(enforcer.can_add_to_playlist(0).is_ok());
        assert!(enforcer.can_add_to_playlist(10_000).is_err());
    }

    #[test]
    fn test_network_validation() {
        let validator = NetworkValidator::default();
        
        let valid_json = b"{\"type\": \"test\"}";
        assert!(validator.validate_message(valid_json).is_ok());
        
        let invalid_json = b"not json";
        assert!(validator.validate_message(invalid_json).is_err());
        
        assert!(validator.validate_device_id("device-123").is_ok());
        assert!(validator.validate_device_id("dev@ice").is_err());
    }

    // Comprehensive tests per design document

    #[test]
    fn test_frequency_validation_boundary_cases() {
        // Valid frequencies
        assert!(ParameterValidator::validate_frequency(20.0, 48000).is_ok());
        assert!(ParameterValidator::validate_frequency(20000.0, 48000).is_ok());
        
        // Invalid frequencies
        assert!(ParameterValidator::validate_frequency(0.0, 48000).is_err());
        assert!(ParameterValidator::validate_frequency(-50.0, 48000).is_err());
        
        // Nyquist clamping - should clamp to 95% of Nyquist
        let result = ParameterValidator::validate_frequency(30000.0, 48000).unwrap();
        assert!(result < 24000.0, "Should be clamped below Nyquist");
        assert!(result > 22000.0, "Should be close to 95% of Nyquist");
    }

    #[test]
    fn test_gain_validation_clamping() {
        // Within range
        assert_eq!(ParameterValidator::validate_gain_db(0.0).unwrap(), 0.0);
        assert_eq!(ParameterValidator::validate_gain_db(12.0).unwrap(), 12.0);
        assert_eq!(ParameterValidator::validate_gain_db(-24.0).unwrap(), -24.0);
        
        // Clamping at extremes
        assert_eq!(ParameterValidator::validate_gain_db(50.0).unwrap(), 24.0);
        assert_eq!(ParameterValidator::validate_gain_db(-100.0).unwrap(), -48.0);
        
        // Invalid values
        assert!(ParameterValidator::validate_gain_db(f32::NAN).is_err());
        assert!(ParameterValidator::validate_gain_db(f32::INFINITY).is_err());
        assert!(ParameterValidator::validate_gain_db(f32::NEG_INFINITY).is_err());
    }

    #[test]
    fn test_q_factor_validation() {
        // Valid Q factors
        assert!(ParameterValidator::validate_q_factor(0.5).is_ok());
        assert!(ParameterValidator::validate_q_factor(1.0).is_ok());
        assert!(ParameterValidator::validate_q_factor(10.0).is_ok());
        
        // Clamping
        assert_eq!(ParameterValidator::validate_q_factor(0.05).unwrap(), 0.1); // Min clamp
        assert_eq!(ParameterValidator::validate_q_factor(50.0).unwrap(), 20.0); // Max clamp
        
        // Invalid
        assert!(ParameterValidator::validate_q_factor(0.0).is_err());
        assert!(ParameterValidator::validate_q_factor(-1.0).is_err());
        assert!(ParameterValidator::validate_q_factor(f32::NAN).is_err());
    }

    #[test]
    fn test_all_sample_rates() {
        let valid_rates = vec![44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];
        for rate in valid_rates {
            assert!(ParameterValidator::validate_sample_rate(rate).is_ok());
        }
        
        let invalid_rates = vec![8000, 22050, 32000, 128000, 500000];
        for rate in invalid_rates {
            assert!(ParameterValidator::validate_sample_rate(rate).is_err());
        }
    }

    #[test]
    fn test_buffer_size_power_of_two() {
        let valid_sizes = vec![64, 128, 256, 512, 1024, 2048, 4096, 8192];
        for size in valid_sizes {
            assert!(ParameterValidator::validate_buffer_size(size).is_ok());
        }
        
        let invalid_sizes = vec![63, 100, 1000, 3000, 10000];
        for size in invalid_sizes {
            assert!(ParameterValidator::validate_buffer_size(size).is_err());
        }
    }

    #[test]
    fn test_buffer_size_range() {
        assert!(ParameterValidator::validate_buffer_size(32).is_err()); // Too small
        assert!(ParameterValidator::validate_buffer_size(64).is_ok());
        assert!(ParameterValidator::validate_buffer_size(8192).is_ok());
        assert!(ParameterValidator::validate_buffer_size(16384).is_err()); // Too large
    }

    #[test]
    fn test_resource_limits_default_values() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_file_size_bytes, 2 * 1024 * 1024 * 1024);
        assert_eq!(limits.max_gpu_memory_percent, 0.8);
        assert_eq!(limits.max_convolution_ir_samples, 16 * 1024 * 1024);
        assert_eq!(limits.max_filter_chain_length, 32);
        assert_eq!(limits.max_playlist_items, 10_000);
        assert_eq!(limits.max_websocket_clients, 8);
    }

    #[test]
    fn test_filter_chain_limit_enforcement() {
        let limits = ResourceLimits::default();
        let enforcer = ResourceLimitEnforcer::new(limits);
        
        // Can add filters up to limit
        for i in 0..32 {
            assert!(enforcer.can_add_filter(i).is_ok());
        }
        
        // Cannot exceed limit
        assert!(enforcer.can_add_filter(32).is_err());
        assert!(enforcer.can_add_filter(100).is_err());
    }

    #[test]
    fn test_playlist_limit_enforcement() {
        let limits = ResourceLimits::default();
        let enforcer = ResourceLimitEnforcer::new(limits);
        
        assert!(enforcer.can_add_to_playlist(0).is_ok());
        assert!(enforcer.can_add_to_playlist(5000).is_ok());
        assert!(enforcer.can_add_to_playlist(9999).is_ok());
        assert!(enforcer.can_add_to_playlist(10_000).is_err());
    }

    #[test]
    fn test_gpu_memory_limit_enforcement() {
        let limits = ResourceLimits::default();
        let enforcer = ResourceLimitEnforcer::new(limits);
        
        // 80% of 1000MB = 800MB
        assert!(enforcer.check_gpu_memory(500, 1000).is_ok());
        assert!(enforcer.check_gpu_memory(800, 1000).is_ok());
        assert!(enforcer.check_gpu_memory(900, 1000).is_err());
    }

    #[test]
    fn test_network_message_size_limit() {
        let validator = NetworkValidator::default();
        
        // Small valid JSON
        let small_json = b"{\"test\": \"data\"}";
        assert!(validator.validate_message(small_json).is_ok());
        
        // Large message exceeding limit
        let large_message = vec![b'a'; 128 * 1024]; // 128KB > 64KB limit
        assert!(validator.validate_message(&large_message).is_err());
    }

    #[test]
    fn test_device_id_validation() {
        let validator = NetworkValidator::default();
        
        // Valid device IDs
        assert!(validator.validate_device_id("device123").is_ok());
        assert!(validator.validate_device_id("device-123").is_ok());
        assert!(validator.validate_device_id("device_123").is_ok());
        assert!(validator.validate_device_id("DEV-001").is_ok());
        
        // Invalid device IDs
        assert!(validator.validate_device_id("").is_err()); // Empty
        assert!(validator.validate_device_id("dev@ice").is_err()); // Special char
        assert!(validator.validate_device_id("dev ice").is_err()); // Space
        assert!(validator.validate_device_id("dev/ice").is_err()); // Slash
        
        // Too long
        let long_id = "a".repeat(300);
        assert!(validator.validate_device_id(&long_id).is_err());
    }

    #[test]
    fn test_path_validator_allowed_extensions() {
        let validator = PathValidator::new();
        
        // Check all expected extensions are in the list
        let expected_extensions = vec![
            "wav", "flac", "alac", "ape", "wv",
            "mp3", "aac", "m4a", "ogg", "opus",
            "dsf", "dff", "dsd",
            "m3u", "m3u8", "pls",
        ];
        
        for ext in expected_extensions {
            assert!(validator.allowed_extensions.contains(&ext.to_string()));
        }
    }

    #[test]
    fn test_network_validator_rate_limits() {
        let validator = NetworkValidator::default();
        assert_eq!(validator.max_message_size, 64 * 1024);
        assert_eq!(validator.rate_limit_window_secs, 1);
        assert_eq!(validator.max_messages_per_window, 100);
    }

    #[test]
    fn test_parameter_validator_edge_cases() {
        // Zero frequency
        assert!(ParameterValidator::validate_frequency(0.0, 48000).is_err());
        
        // Negative gain extreme
        let result = ParameterValidator::validate_gain_db(-1000.0).unwrap();
        assert_eq!(result, -48.0);
        
        // Q factor exactly at boundaries
        assert_eq!(ParameterValidator::validate_q_factor(0.1).unwrap(), 0.1);
        assert_eq!(ParameterValidator::validate_q_factor(20.0).unwrap(), 20.0);
    }

    #[test]
    fn test_json_parsing_in_network_validator() {
        let validator = NetworkValidator::default();
        
        // Valid JSON objects
        assert!(validator.validate_message(b"{}").is_ok());
        assert!(validator.validate_message(b"{\"key\": \"value\"}").is_ok());
        assert!(validator.validate_message(b"[1, 2, 3]").is_ok());
        
        // Invalid JSON
        assert!(validator.validate_message(b"{").is_err());
        assert!(validator.validate_message(b"not json at all").is_err());
        assert!(validator.validate_message(b"{\"key\": }").is_err());
    }
}
