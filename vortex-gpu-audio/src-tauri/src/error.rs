/// Comprehensive error handling framework for Vortex GPU Audio
/// 
/// This module implements layered error handling with automatic recovery mechanisms
/// as specified in the design review document (Section 10).

use thiserror::Error;
use std::fmt;

/// Top-level error type for the entire application
#[derive(Debug, Error)]
pub enum VortexError {
    /// Audio driver and processing errors (Critical severity)
    #[error("Audio error: {0}")]
    Audio(#[from] AudioError),

    /// GPU processing errors (High severity)
    #[error("GPU error: {0}")]
    Gpu(#[from] GpuError),

    /// File I/O errors (Medium severity)
    #[error("File I/O error: {0}")]
    FileIo(#[from] FileIoError),

    /// Network and device discovery errors (Low severity)
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    /// Configuration and validation errors
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// FFI boundary errors
    #[error("FFI error: {0}")]
    Ffi(#[from] FfiError),
}

/// Audio subsystem errors with automatic recovery strategies
#[derive(Debug, Error)]
pub enum AudioError {
    /// Audio driver initialization failure
    #[error("Failed to initialize audio driver '{driver}': {reason}")]
    DriverInitFailed {
        driver: String,
        reason: String,
    },

    /// Audio driver runtime failure
    #[error("Audio driver '{driver}' failed during operation: {reason}")]
    DriverRuntimeError {
        driver: String,
        reason: String,
    },

    /// Buffer underrun detected
    #[error("Audio buffer underrun detected: {samples_lost} samples lost")]
    BufferUnderrun {
        samples_lost: usize,
    },

    /// Processing latency exceeded budget
    #[error("Processing latency {actual_ms}ms exceeded budget {budget_ms}ms")]
    LatencyExceeded {
        actual_ms: f64,
        budget_ms: f64,
    },

    /// Invalid audio configuration
    #[error("Invalid audio configuration: {reason}")]
    InvalidConfig {
        reason: String,
    },

    /// No audio devices available
    #[error("No audio devices available")]
    NoDevicesAvailable,
}

impl AudioError {
    /// Determine if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            AudioError::DriverInitFailed { .. } => true,  // Can try alternative driver
            AudioError::DriverRuntimeError { .. } => true, // Can attempt driver reset
            AudioError::BufferUnderrun { .. } => true,     // Can adjust buffer size
            AudioError::LatencyExceeded { .. } => true,    // Can optimize or fallback
            AudioError::InvalidConfig { .. } => false,     // Requires user intervention
            AudioError::NoDevicesAvailable => false,       // Cannot recover automatically
        }
    }

    /// Get the severity level of this error
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            AudioError::DriverInitFailed { .. } => ErrorSeverity::Critical,
            AudioError::DriverRuntimeError { .. } => ErrorSeverity::Critical,
            AudioError::BufferUnderrun { .. } => ErrorSeverity::High,
            AudioError::LatencyExceeded { .. } => ErrorSeverity::High,
            AudioError::InvalidConfig { .. } => ErrorSeverity::Medium,
            AudioError::NoDevicesAvailable => ErrorSeverity::Critical,
        }
    }
}

/// GPU processing errors with fallback strategies
#[derive(Debug, Error)]
pub enum GpuError {
    /// GPU initialization failed
    #[error("Failed to initialize GPU backend '{backend}': {reason}")]
    InitializationFailed {
        backend: String,
        reason: String,
    },

    /// GPU memory allocation failed
    #[error("GPU memory allocation failed: requested {requested_bytes} bytes, available {available_bytes} bytes")]
    MemoryAllocationFailed {
        requested_bytes: usize,
        available_bytes: usize,
    },

    /// GPU kernel execution failed
    #[error("GPU kernel '{kernel_name}' execution failed: {reason}")]
    KernelExecutionFailed {
        kernel_name: String,
        reason: String,
    },

    /// GPU memory transfer failed
    #[error("GPU memory transfer failed: {reason}")]
    MemoryTransferFailed {
        reason: String,
    },

    /// GPU not available
    #[error("No compatible GPU found for backend '{backend}'")]
    NoGpuAvailable {
        backend: String,
    },
}

impl GpuError {
    /// Determine if CPU fallback is possible
    pub fn can_fallback_to_cpu(&self) -> bool {
        match self {
            GpuError::InitializationFailed { .. } => true,
            GpuError::MemoryAllocationFailed { .. } => true,
            GpuError::KernelExecutionFailed { .. } => true,
            GpuError::MemoryTransferFailed { .. } => true,
            GpuError::NoGpuAvailable { .. } => true,
        }
    }

    pub fn severity(&self) -> ErrorSeverity {
        ErrorSeverity::High // GPU errors degrade performance but don't stop operation
    }
}

/// File I/O errors
#[derive(Debug, Error)]
pub enum FileIoError {
    /// File not found
    #[error("File not found: {path}")]
    FileNotFound {
        path: String,
    },

    /// Unsupported file format
    #[error("Unsupported file format: {format} (file: {path})")]
    UnsupportedFormat {
        format: String,
        path: String,
    },

    /// File size exceeds limit
    #[error("File size {size_bytes} bytes exceeds limit {limit_bytes} bytes")]
    FileSizeExceeded {
        size_bytes: u64,
        limit_bytes: u64,
    },

    /// File corruption detected
    #[error("File corruption detected in {path}: {reason}")]
    FileCorrupted {
        path: String,
        reason: String,
    },

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl FileIoError {
    pub fn severity(&self) -> ErrorSeverity {
        ErrorSeverity::Medium // File errors affect single file, not entire system
    }
}

/// Network and discovery errors
#[derive(Debug, Error)]
pub enum NetworkError {
    /// Device discovery failed
    #[error("Device discovery failed: {reason}")]
    DiscoveryFailed {
        reason: String,
    },

    /// WebSocket connection error
    #[error("WebSocket error: {reason}")]
    WebSocketError {
        reason: String,
    },

    /// Invalid network message
    #[error("Invalid network message: {reason}")]
    InvalidMessage {
        reason: String,
    },
}

impl NetworkError {
    pub fn severity(&self) -> ErrorSeverity {
        ErrorSeverity::Low // Network errors don't affect core audio processing
    }
}

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Invalid configuration value
    #[error("Invalid configuration value for '{key}': {reason}")]
    InvalidValue {
        key: String,
        reason: String,
    },

    /// Missing required configuration
    #[error("Missing required configuration: {key}")]
    MissingRequired {
        key: String,
    },

    /// Configuration parsing error
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),
}

/// FFI boundary errors
#[derive(Debug, Error)]
pub enum FfiError {
    /// Null pointer encountered
    #[error("Null pointer encountered in FFI call: {context}")]
    NullPointer {
        context: String,
    },

    /// Invalid buffer size
    #[error("Invalid buffer size: expected {expected}, got {actual}")]
    InvalidBufferSize {
        expected: usize,
        actual: usize,
    },

    /// C++ exception caught
    #[error("C++ exception: {message}")]
    CppException {
        message: String,
    },

    /// Memory alignment error
    #[error("Memory alignment error: expected alignment {expected}, got {actual}")]
    AlignmentError {
        expected: usize,
        actual: usize,
    },
}

/// Error severity levels for routing and recovery
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    /// Low severity - Silent retry, log warning
    Low,
    /// Medium severity - Display error, continue with playlist
    Medium,
    /// High severity - Performance degradation warning
    High,
    /// Critical severity - Notification + auto-recovery attempt
    Critical,
}

/// Error recovery strategy
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// No recovery possible, notify user
    NoRecovery,
    /// Retry with exponential backoff
    RetryWithBackoff {
        max_attempts: u32,
        initial_delay_ms: u64,
    },
    /// Fallback to alternative implementation
    Fallback {
        description: String,
    },
    /// Reset and restart component
    Reset {
        component: String,
    },
}

/// Error context for enriching error information
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub component: String,
    pub operation: String,
    pub timestamp: std::time::SystemTime,
    pub additional_info: Option<String>,
}

impl ErrorContext {
    pub fn new(component: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            operation: operation.into(),
            timestamp: std::time::SystemTime::now(),
            additional_info: None,
        }
    }

    pub fn with_info(mut self, info: impl Into<String>) -> Self {
        self.additional_info = Some(info.into());
        self
    }
}

/// Result type alias for convenience
pub type VortexResult<T> = Result<T, VortexError>;

/// Error handler trait for implementing recovery strategies
pub trait ErrorHandler {
    /// Handle an error with automatic recovery
    fn handle_error(&self, error: &VortexError, context: &ErrorContext) -> RecoveryStrategy;
    
    /// Log error with appropriate level
    fn log_error(&self, error: &VortexError, context: &ErrorContext);
    
    /// Notify user if necessary
    fn notify_user(&self, error: &VortexError, severity: ErrorSeverity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_error_severity() {
        let err = AudioError::DriverInitFailed {
            driver: "WASAPI".to_string(),
            reason: "Device not found".to_string(),
        };
        assert_eq!(err.severity(), ErrorSeverity::Critical);
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_gpu_error_fallback() {
        let err = GpuError::NoGpuAvailable {
            backend: "CUDA".to_string(),
        };
        assert!(err.can_fallback_to_cpu());
        assert_eq!(err.severity(), ErrorSeverity::High);
    }

    #[test]
    fn test_error_context() {
        let ctx = ErrorContext::new("AudioEngine", "initialize")
            .with_info("Sample rate: 48000");
        assert_eq!(ctx.component, "AudioEngine");
        assert_eq!(ctx.operation, "initialize");
        assert!(ctx.additional_info.is_some());
    }

    // Comprehensive tests per design document

    #[test]
    fn test_all_audio_errors_severity() {
        let errors = vec![
            AudioError::DriverInitFailed {
                driver: "test".into(),
                reason: "test".into(),
            },
            AudioError::DriverRuntimeError {
                driver: "test".into(),
                reason: "test".into(),
            },
            AudioError::BufferUnderrun { samples_lost: 100 },
            AudioError::LatencyExceeded {
                actual_ms: 10.0,
                budget_ms: 5.0,
            },
            AudioError::InvalidConfig {
                reason: "test".into(),
            },
            AudioError::NoDevicesAvailable,
        ];

        // Verify severity levels
        assert_eq!(errors[0].severity(), ErrorSeverity::Critical);
        assert_eq!(errors[1].severity(), ErrorSeverity::Critical);
        assert_eq!(errors[2].severity(), ErrorSeverity::High);
        assert_eq!(errors[3].severity(), ErrorSeverity::High);
        assert_eq!(errors[4].severity(), ErrorSeverity::Medium);
        assert_eq!(errors[5].severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_audio_error_recoverability() {
        assert!(AudioError::DriverInitFailed {
            driver: "test".into(),
            reason: "test".into(),
        }
        .is_recoverable());

        assert!(AudioError::BufferUnderrun { samples_lost: 100 }.is_recoverable());

        assert!(!AudioError::InvalidConfig {
            reason: "test".into(),
        }
        .is_recoverable());

        assert!(!AudioError::NoDevicesAvailable.is_recoverable());
    }

    #[test]
    fn test_all_gpu_errors_can_fallback() {
        let errors = vec![
            GpuError::InitializationFailed {
                backend: "CUDA".into(),
                reason: "not found".into(),
            },
            GpuError::MemoryAllocationFailed {
                requested_bytes: 1000,
                available_bytes: 500,
            },
            GpuError::KernelExecutionFailed {
                kernel_name: "convolution".into(),
                reason: "timeout".into(),
            },
            GpuError::MemoryTransferFailed {
                reason: "DMA error".into(),
            },
            GpuError::NoGpuAvailable {
                backend: "Vulkan".into(),
            },
        ];

        // All GPU errors should allow CPU fallback
        for err in errors {
            assert!(err.can_fallback_to_cpu());
            assert_eq!(err.severity(), ErrorSeverity::High);
        }
    }

    #[test]
    fn test_file_io_error_severity() {
        let errors = vec![
            FileIoError::FileNotFound {
                path: "test.wav".into(),
            },
            FileIoError::UnsupportedFormat {
                format: "xyz".into(),
                path: "test.xyz".into(),
            },
            FileIoError::FileSizeExceeded {
                size_bytes: 3_000_000_000,
                limit_bytes: 2_000_000_000,
            },
            FileIoError::FileCorrupted {
                path: "test.wav".into(),
                reason: "invalid header".into(),
            },
        ];

        for err in errors {
            assert_eq!(err.severity(), ErrorSeverity::Medium);
        }
    }

    #[test]
    fn test_network_error_severity() {
        let err = NetworkError::DiscoveryFailed {
            reason: "timeout".into(),
        };
        assert_eq!(err.severity(), ErrorSeverity::Low);

        let err2 = NetworkError::WebSocketError {
            reason: "connection closed".into(),
        };
        assert_eq!(err2.severity(), ErrorSeverity::Low);
    }

    #[test]
    fn test_error_conversion_chain() {
        // Test From trait implementations
        let audio_err = AudioError::NoDevicesAvailable;
        let vortex_err: VortexError = audio_err.into();
        assert!(matches!(vortex_err, VortexError::Audio(_)));

        let gpu_err = GpuError::NoGpuAvailable {
            backend: "CUDA".into(),
        };
        let vortex_err2: VortexError = gpu_err.into();
        assert!(matches!(vortex_err2, VortexError::Gpu(_)));

        let file_err = FileIoError::FileNotFound {
            path: "test".into(),
        };
        let vortex_err3: VortexError = file_err.into();
        assert!(matches!(vortex_err3, VortexError::FileIo(_)));
    }

    #[test]
    fn test_error_context_builder() {
        let ctx1 = ErrorContext::new("Component", "operation");
        assert_eq!(ctx1.component, "Component");
        assert_eq!(ctx1.operation, "operation");
        assert!(ctx1.additional_info.is_none());

        let ctx2 = ctx1.with_info("extra info");
        assert_eq!(ctx2.additional_info, Some("extra info".to_string()));
    }

    #[test]
    fn test_error_context_timestamp() {
        let ctx = ErrorContext::new("test", "test");
        let now = std::time::SystemTime::now();
        
        // Timestamp should be very recent
        let duration = now.duration_since(ctx.timestamp).unwrap();
        assert!(duration.as_secs() < 1);
    }

    #[test]
    fn test_error_severity_ordering() {
        assert!(ErrorSeverity::Low < ErrorSeverity::Medium);
        assert!(ErrorSeverity::Medium < ErrorSeverity::High);
        assert!(ErrorSeverity::High < ErrorSeverity::Critical);
    }

    #[test]
    fn test_error_display_messages() {
        let err = AudioError::DriverInitFailed {
            driver: "WASAPI".to_string(),
            reason: "Device not found".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("WASAPI"));
        assert!(msg.contains("Device not found"));

        let gpu_err = GpuError::MemoryAllocationFailed {
            requested_bytes: 1000,
            available_bytes: 500,
        };
        let gpu_msg = format!("{}", gpu_err);
        assert!(gpu_msg.contains("1000"));
        assert!(gpu_msg.contains("500"));
    }

    #[test]
    fn test_buffer_underrun_details() {
        let err = AudioError::BufferUnderrun { samples_lost: 256 };
        let msg = format!("{}", err);
        assert!(msg.contains("256"));
        assert!(msg.contains("samples lost"));
        assert_eq!(err.severity(), ErrorSeverity::High);
    }

    #[test]
    fn test_latency_exceeded_details() {
        let err = AudioError::LatencyExceeded {
            actual_ms: 10.5,
            budget_ms: 5.0,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("10.5"));
        assert!(msg.contains("5"));
    }

    #[test]
    fn test_config_error_types() {
        let err1 = ConfigError::InvalidValue {
            key: "sample_rate".into(),
            reason: "must be positive".into(),
        };
        let msg1 = format!("{}", err1);
        assert!(msg1.contains("sample_rate"));

        let err2 = ConfigError::MissingRequired {
            key: "buffer_size".into(),
        };
        let msg2 = format!("{}", err2);
        assert!(msg2.contains("buffer_size"));
    }

    #[test]
    fn test_ffi_error_types() {
        let err = FfiError::NullPointer {
            context: "audio buffer".into(),
        };
        assert!(format!("{}", err).contains("audio buffer"));

        let err2 = FfiError::InvalidBufferSize {
            expected: 1024,
            actual: 512,
        };
        assert!(format!("{}", err2).contains("1024"));
        assert!(format!("{}", err2).contains("512"));
    }
}
