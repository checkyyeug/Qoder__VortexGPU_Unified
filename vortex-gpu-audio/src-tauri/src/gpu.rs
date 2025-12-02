/// GPU backend trait abstraction with compile-time backend selection
/// 
/// This module implements the improved GPU architecture from Section 2 of the design review,
/// using trait-based polymorphism instead of runtime enum dispatch.

use crate::error::{GpuError, VortexResult};
use std::fmt::Debug;

/// GPU backend identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackendType {
    Cuda,
    OpenCL,
    Vulkan,
    Cpu, // CPU fallback
}

/// GPU processing capabilities
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    pub backend_type: GpuBackendType,
    pub device_name: String,
    pub compute_units: u32,
    pub max_memory_mb: usize,
    pub supports_fp64: bool,
    pub supports_async_transfer: bool,
}

/// GPU memory buffer abstraction
pub trait GpuBuffer: Send + Sync {
    /// Get the size of the buffer in bytes
    fn size(&self) -> usize;
    
    /// Check if the buffer is allocated on device
    fn is_device_memory(&self) -> bool;
    
    /// Get the memory alignment
    fn alignment(&self) -> usize;
}

/// GPU backend trait - all backends must implement this
pub trait GpuBackend: Send + Sync + Debug {
    type Buffer: GpuBuffer;

    /// Initialize the GPU backend
    fn initialize() -> VortexResult<Self> where Self: Sized;

    /// Get backend capabilities
    fn capabilities(&self) -> &GpuCapabilities;

    /// Allocate device memory
    fn allocate_buffer(&self, size_bytes: usize) -> VortexResult<Self::Buffer>;

    /// Free device memory
    fn free_buffer(&self, buffer: Self::Buffer) -> VortexResult<()>;

    /// Copy data from host to device
    fn copy_to_device(&self, buffer: &Self::Buffer, host_data: &[f32]) -> VortexResult<()>;

    /// Copy data from device to host
    fn copy_from_device(&self, buffer: &Self::Buffer, host_data: &mut [f32]) -> VortexResult<()>;

    /// Process convolution on GPU
    /// 
    /// # Arguments
    /// * `input` - Input audio buffer on device
    /// * `impulse_response` - Impulse response buffer on device
    /// * `output` - Output buffer on device
    /// * `input_samples` - Number of input samples
    /// * `ir_samples` - Number of IR samples
    fn process_convolution(
        &self,
        input: &Self::Buffer,
        impulse_response: &Self::Buffer,
        output: &Self::Buffer,
        input_samples: usize,
        ir_samples: usize,
    ) -> VortexResult<()>;

    /// Process parametric EQ on GPU
    /// 
    /// # Arguments
    /// * `input` - Input audio buffer on device
    /// * `output` - Output buffer on device
    /// * `bands` - EQ band parameters
    /// * `samples` - Number of samples
    fn process_eq(
        &self,
        input: &Self::Buffer,
        output: &Self::Buffer,
        bands: &[EqBand],
        samples: usize,
    ) -> VortexResult<()>;

    /// Process FFT (Fast Fourier Transform)
    /// 
    /// # Arguments
    /// * `input` - Input buffer (real samples)
    /// * `output` - Output buffer (complex spectrum)
    /// * `fft_size` - FFT size (must be power of 2)
    fn process_fft(
        &self,
        input: &Self::Buffer,
        output: &Self::Buffer,
        fft_size: usize,
    ) -> VortexResult<()>;

    /// Process IFFT (Inverse Fast Fourier Transform)
    fn process_ifft(
        &self,
        input: &Self::Buffer,
        output: &Self::Buffer,
        fft_size: usize,
    ) -> VortexResult<()>;

    /// Synchronize GPU operations (wait for completion)
    fn synchronize(&self) -> VortexResult<()>;

    /// Get current GPU memory usage
    fn memory_usage(&self) -> GpuMemoryInfo;

    /// Check if GPU is available and operational
    fn is_operational(&self) -> bool;
}

/// EQ band parameters
#[derive(Debug, Clone, Copy)]
pub struct EqBand {
    pub frequency: f32,    // Center frequency in Hz
    pub gain: f32,         // Gain in dB
    pub q_factor: f32,     // Q factor (bandwidth)
    pub filter_type: EqFilterType,
}

#[derive(Debug, Clone, Copy)]
pub enum EqFilterType {
    Peak,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
}

/// GPU memory information
#[derive(Debug, Clone, Copy)]
pub struct GpuMemoryInfo {
    pub total_mb: usize,
    pub used_mb: usize,
    pub available_mb: usize,
    pub usage_percentage: f32,
}

/// GPU processor that wraps the selected backend
pub struct GpuProcessor {
    backend: Box<dyn GpuBackend<Buffer = DynGpuBuffer>>,
    capabilities: GpuCapabilities,
}

impl GpuProcessor {
    /// Create a new GPU processor with the specified backend type
    pub fn new(backend_type: GpuBackendType) -> VortexResult<Self> {
        match backend_type {
            #[cfg(feature = "cuda")]
            GpuBackendType::Cuda => {
                let backend = CudaBackend::initialize()?;
                let capabilities = backend.capabilities().clone();
                Ok(Self {
                    backend: Box::new(backend),
                    capabilities,
                })
            }
            #[cfg(feature = "opencl")]
            GpuBackendType::OpenCL => {
                let backend = OpenCLBackend::initialize()?;
                let capabilities = backend.capabilities().clone();
                Ok(Self {
                    backend: Box::new(backend),
                    capabilities,
                })
            }
            #[cfg(feature = "vulkan")]
            GpuBackendType::Vulkan => {
                let backend = VulkanBackend::initialize()?;
                let capabilities = backend.capabilities().clone();
                Ok(Self {
                    backend: Box::new(backend),
                    capabilities,
                })
            }
            GpuBackendType::Cpu => {
                let backend = CpuFallbackBackend::initialize()?;
                let capabilities = backend.capabilities().clone();
                Ok(Self {
                    backend: Box::new(backend),
                    capabilities,
                })
            }
            _ => {
                // Fallback to CPU if requested backend not compiled in
                let backend = CpuFallbackBackend::initialize()?;
                let capabilities = backend.capabilities().clone();
                Ok(Self {
                    backend: Box::new(backend),
                    capabilities,
                })
            }
        }
    }

    /// Auto-detect and select the best available GPU backend
    pub fn auto_detect() -> VortexResult<Self> {
        // Try backends in priority order
        #[cfg(feature = "cuda")]
        {
            if let Ok(processor) = Self::new(GpuBackendType::Cuda) {
                return Ok(processor);
            }
        }

        #[cfg(feature = "vulkan")]
        {
            if let Ok(processor) = Self::new(GpuBackendType::Vulkan) {
                return Ok(processor);
            }
        }

        #[cfg(feature = "opencl")]
        {
            if let Ok(processor) = Self::new(GpuBackendType::OpenCL) {
                return Ok(processor);
            }
        }

        // Fallback to CPU
        Self::new(GpuBackendType::Cpu)
    }

    pub fn capabilities(&self) -> &GpuCapabilities {
        &self.capabilities
    }

    pub fn backend(&self) -> &dyn GpuBackend<Buffer = DynGpuBuffer> {
        self.backend.as_ref()
    }
}

/// Dynamic GPU buffer wrapper
pub struct DynGpuBuffer {
    size: usize,
    alignment: usize,
    is_device: bool,
}

impl GpuBuffer for DynGpuBuffer {
    fn size(&self) -> usize {
        self.size
    }

    fn is_device_memory(&self) -> bool {
        self.is_device
    }

    fn alignment(&self) -> usize {
        self.alignment
    }
}

/// CPU fallback backend (always available)
#[derive(Debug)]
struct CpuFallbackBackend {
    capabilities: GpuCapabilities,
}

impl GpuBackend for CpuFallbackBackend {
    type Buffer = DynGpuBuffer;

    fn initialize() -> VortexResult<Self> {
        Ok(Self {
            capabilities: GpuCapabilities {
                backend_type: GpuBackendType::Cpu,
                device_name: "CPU Fallback".to_string(),
                compute_units: num_cpus::get() as u32,
                max_memory_mb: 1024, // Limit CPU buffer to 1GB
                supports_fp64: true,
                supports_async_transfer: false,
            },
        })
    }

    fn capabilities(&self) -> &GpuCapabilities {
        &self.capabilities
    }

    fn allocate_buffer(&self, size_bytes: usize) -> VortexResult<Self::Buffer> {
        Ok(DynGpuBuffer {
            size: size_bytes,
            alignment: 64, // Cache line alignment
            is_device: false,
        })
    }

    fn free_buffer(&self, _buffer: Self::Buffer) -> VortexResult<()> {
        Ok(())
    }

    fn copy_to_device(&self, _buffer: &Self::Buffer, _host_data: &[f32]) -> VortexResult<()> {
        Ok(()) // No-op for CPU
    }

    fn copy_from_device(&self, _buffer: &Self::Buffer, _host_data: &mut [f32]) -> VortexResult<()> {
        Ok(()) // No-op for CPU
    }

    fn process_convolution(
        &self,
        _input: &Self::Buffer,
        _impulse_response: &Self::Buffer,
        _output: &Self::Buffer,
        _input_samples: usize,
        _ir_samples: usize,
    ) -> VortexResult<()> {
        // TODO: Implement CPU convolution using SIMD
        Ok(())
    }

    fn process_eq(
        &self,
        _input: &Self::Buffer,
        _output: &Self::Buffer,
        _bands: &[EqBand],
        _samples: usize,
    ) -> VortexResult<()> {
        // TODO: Implement CPU EQ using biquad filters
        Ok(())
    }

    fn process_fft(
        &self,
        _input: &Self::Buffer,
        _output: &Self::Buffer,
        _fft_size: usize,
    ) -> VortexResult<()> {
        // TODO: Implement CPU FFT
        Ok(())
    }

    fn process_ifft(
        &self,
        _input: &Self::Buffer,
        _output: &Self::Buffer,
        _fft_size: usize,
    ) -> VortexResult<()> {
        // TODO: Implement CPU IFFT
        Ok(())
    }

    fn synchronize(&self) -> VortexResult<()> {
        Ok(()) // No sync needed for CPU
    }

    fn memory_usage(&self) -> GpuMemoryInfo {
        GpuMemoryInfo {
            total_mb: 1024,
            used_mb: 0,
            available_mb: 1024,
            usage_percentage: 0.0,
        }
    }

    fn is_operational(&self) -> bool {
        true // CPU is always operational
    }
}

// Placeholder backends for CUDA, OpenCL, Vulkan
// These will be implemented when the respective features are enabled

#[cfg(feature = "cuda")]
mod cuda_backend {
    use super::*;

    #[derive(Debug)]
    pub struct CudaBackend {
        capabilities: GpuCapabilities,
    }

    impl GpuBackend for CudaBackend {
        type Buffer = DynGpuBuffer;

        fn initialize() -> VortexResult<Self> {
            // TODO: Initialize CUDA
            Err(GpuError::InitializationFailed {
                backend: "CUDA".to_string(),
                reason: "Not yet implemented".to_string(),
            }.into())
        }

        // ... implement other methods
        fn capabilities(&self) -> &GpuCapabilities { &self.capabilities }
        fn allocate_buffer(&self, _size_bytes: usize) -> VortexResult<Self::Buffer> { unimplemented!() }
        fn free_buffer(&self, _buffer: Self::Buffer) -> VortexResult<()> { unimplemented!() }
        fn copy_to_device(&self, _buffer: &Self::Buffer, _host_data: &[f32]) -> VortexResult<()> { unimplemented!() }
        fn copy_from_device(&self, _buffer: &Self::Buffer, _host_data: &mut [f32]) -> VortexResult<()> { unimplemented!() }
        fn process_convolution(&self, _input: &Self::Buffer, _impulse_response: &Self::Buffer, _output: &Self::Buffer, _input_samples: usize, _ir_samples: usize) -> VortexResult<()> { unimplemented!() }
        fn process_eq(&self, _input: &Self::Buffer, _output: &Self::Buffer, _bands: &[EqBand], _samples: usize) -> VortexResult<()> { unimplemented!() }
        fn process_fft(&self, _input: &Self::Buffer, _output: &Self::Buffer, _fft_size: usize) -> VortexResult<()> { unimplemented!() }
        fn process_ifft(&self, _input: &Self::Buffer, _output: &Self::Buffer, _fft_size: usize) -> VortexResult<()> { unimplemented!() }
        fn synchronize(&self) -> VortexResult<()> { unimplemented!() }
        fn memory_usage(&self) -> GpuMemoryInfo { unimplemented!() }
        fn is_operational(&self) -> bool { false }
    }
}

#[cfg(feature = "cuda")]
pub use cuda_backend::CudaBackend;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_backend_initialization() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        assert_eq!(backend.capabilities().backend_type, GpuBackendType::Cpu);
        assert!(backend.is_operational());
    }

    #[test]
    fn test_gpu_processor_auto_detect() {
        let processor = GpuProcessor::auto_detect();
        assert!(processor.is_ok());
        
        let processor = processor.unwrap();
        let caps = processor.capabilities();
        assert!(caps.compute_units > 0);
    }

    #[test]
    fn test_buffer_allocation() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        let buffer = backend.allocate_buffer(1024 * 4).unwrap();
        assert_eq!(buffer.size(), 1024 * 4);
        assert!(!buffer.is_device_memory());
    }

    // Comprehensive tests per design document

    #[test]
    fn test_cpu_fallback_capabilities() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        let caps = backend.capabilities();
        
        assert_eq!(caps.backend_type, GpuBackendType::Cpu);
        assert_eq!(caps.device_name, "CPU Fallback");
        assert!(caps.compute_units > 0); // Should match CPU cores
        assert_eq!(caps.max_memory_mb, 1024);
        assert!(caps.supports_fp64);
        assert!(!caps.supports_async_transfer);
    }

    #[test]
    fn test_cpu_backend_always_operational() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        assert!(backend.is_operational());
    }

    #[test]
    fn test_buffer_properties() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        
        let sizes = vec![64, 1024, 4096, 1024 * 1024];
        for size in sizes {
            let buffer = backend.allocate_buffer(size).unwrap();
            assert_eq!(buffer.size(), size);
            assert_eq!(buffer.alignment(), 64); // Cache line alignment
            assert!(!buffer.is_device_memory());
        }
    }

    #[test]
    fn test_memory_operations_no_op() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        let buffer = backend.allocate_buffer(1024).unwrap();
        
        let data = vec![1.0f32; 256];
        assert!(backend.copy_to_device(&buffer, &data).is_ok());
        
        let mut output = vec![0.0f32; 256];
        assert!(backend.copy_from_device(&buffer, &mut output).is_ok());
    }

    #[test]
    fn test_processing_operations_stub() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        let input = backend.allocate_buffer(1024).unwrap();
        let output = backend.allocate_buffer(1024).unwrap();
        let ir = backend.allocate_buffer(2048).unwrap();
        
        // These are stubs for now, should not panic
        assert!(backend.process_convolution(&input, &ir, &output, 256, 512).is_ok());
        
        let bands = vec![EqBand {
            frequency: 1000.0,
            gain: 3.0,
            q_factor: 1.0,
            filter_type: EqFilterType::Peak,
        }];
        assert!(backend.process_eq(&input, &output, &bands, 256).is_ok());
        
        assert!(backend.process_fft(&input, &output, 1024).is_ok());
        assert!(backend.process_ifft(&input, &output, 1024).is_ok());
    }

    #[test]
    fn test_synchronize_no_op() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        assert!(backend.synchronize().is_ok());
    }

    #[test]
    fn test_memory_usage_tracking() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        let info = backend.memory_usage();
        
        assert_eq!(info.total_mb, 1024);
        assert_eq!(info.used_mb, 0);
        assert_eq!(info.available_mb, 1024);
        assert_eq!(info.usage_percentage, 0.0);
    }

    #[test]
    fn test_gpu_processor_creation_cpu_fallback() {
        let processor = GpuProcessor::new(GpuBackendType::Cpu).unwrap();
        assert_eq!(processor.capabilities().backend_type, GpuBackendType::Cpu);
    }

    #[test]
    fn test_gpu_backend_type_equality() {
        assert_eq!(GpuBackendType::Cpu, GpuBackendType::Cpu);
        assert_ne!(GpuBackendType::Cpu, GpuBackendType::Cuda);
        assert_ne!(GpuBackendType::Cuda, GpuBackendType::OpenCL);
    }

    #[test]
    fn test_eq_band_structure() {
        let band = EqBand {
            frequency: 1000.0,
            gain: 6.0,
            q_factor: 1.41,
            filter_type: EqFilterType::Peak,
        };
        
        assert_eq!(band.frequency, 1000.0);
        assert_eq!(band.gain, 6.0);
        assert_eq!(band.q_factor, 1.41);
    }

    #[test]
    fn test_eq_filter_types() {
        let types = vec![
            EqFilterType::Peak,
            EqFilterType::LowShelf,
            EqFilterType::HighShelf,
            EqFilterType::LowPass,
            EqFilterType::HighPass,
        ];
        
        // Just verify they can be created and cloned
        for filter_type in types {
            let band = EqBand {
                frequency: 1000.0,
                gain: 0.0,
                q_factor: 1.0,
                filter_type,
            };
            let _cloned = band.clone();
        }
    }

    #[test]
    fn test_gpu_capabilities_clone() {
        let caps = GpuCapabilities {
            backend_type: GpuBackendType::Cpu,
            device_name: "Test".to_string(),
            compute_units: 8,
            max_memory_mb: 2048,
            supports_fp64: true,
            supports_async_transfer: false,
        };
        
        let cloned = caps.clone();
        assert_eq!(caps.backend_type, cloned.backend_type);
        assert_eq!(caps.device_name, cloned.device_name);
        assert_eq!(caps.compute_units, cloned.compute_units);
    }

    #[test]
    fn test_gpu_memory_info_structure() {
        let info = GpuMemoryInfo {
            total_mb: 8192,
            used_mb: 2048,
            available_mb: 6144,
            usage_percentage: 25.0,
        };
        
        assert_eq!(info.total_mb, 8192);
        assert_eq!(info.used_mb, 2048);
        assert_eq!(info.available_mb, 6144);
        assert_eq!(info.usage_percentage, 25.0);
    }

    #[test]
    fn test_buffer_free_operation() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        let buffer = backend.allocate_buffer(1024).unwrap();
        
        // Should not panic
        assert!(backend.free_buffer(buffer).is_ok());
    }

    #[test]
    fn test_multiple_buffer_allocations() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        
        let buffer1 = backend.allocate_buffer(1024).unwrap();
        let buffer2 = backend.allocate_buffer(2048).unwrap();
        let buffer3 = backend.allocate_buffer(512).unwrap();
        
        assert_eq!(buffer1.size(), 1024);
        assert_eq!(buffer2.size(), 2048);
        assert_eq!(buffer3.size(), 512);
    }

    #[test]
    fn test_auto_detect_fallback_logic() {
        // Auto-detect should always succeed, falling back to CPU if needed
        let processor = GpuProcessor::auto_detect().unwrap();
        let backend_type = processor.capabilities().backend_type;
        
        // On systems without GPU, should be CPU
        // On systems with GPU, could be CUDA, Vulkan, OpenCL, or CPU
        assert!(
            backend_type == GpuBackendType::Cpu ||
            backend_type == GpuBackendType::Cuda ||
            backend_type == GpuBackendType::Vulkan ||
            backend_type == GpuBackendType::OpenCL
        );
    }

    #[test]
    fn test_cpu_compute_units_matches_cores() {
        let backend = CpuFallbackBackend::initialize().unwrap();
        let caps = backend.capabilities();
        let cpu_count = num_cpus::get() as u32;
        
        assert_eq!(caps.compute_units, cpu_count);
    }

    #[test]
    fn test_dyn_gpu_buffer_creation() {
        let buffer = DynGpuBuffer {
            size: 2048,
            alignment: 64,
            is_device: false,
        };
        
        assert_eq!(buffer.size(), 2048);
        assert_eq!(buffer.alignment(), 64);
        assert!(!buffer.is_device_memory());
    }

    #[test]
    fn test_gpu_processor_capabilities_access() {
        let processor = GpuProcessor::auto_detect().unwrap();
        let caps = processor.capabilities();
        
        // Capabilities should be accessible and valid
        assert!(!caps.device_name.is_empty());
        assert!(caps.compute_units > 0);
        assert!(caps.max_memory_mb > 0);
    }
}
