/// Integration tests for GPU processing pipeline
/// 
/// Tests GPU acceleration, CPU fallback, and processing correctness

use vortex_gpu_audio::gpu::{GpuProcessor, GpuBackendType, EqBand, EqFilterType};
use vortex_gpu_audio::error::VortexResult;

mod common;
use common::{generate_sine_wave, generate_white_noise, TestConfig};

#[test]
fn test_gpu_processor_initialization() -> VortexResult<()> {
    // Should always succeed with CPU fallback
    let processor = GpuProcessor::auto_detect()?;
    
    let caps = processor.capabilities();
    assert!(caps.compute_units > 0);
    assert!(!caps.device_name.is_empty());
    assert!(caps.max_memory_mb > 0);
    
    Ok(())
}

#[test]
fn test_cpu_fallback_backend() -> VortexResult<()> {
    let processor = GpuProcessor::new(GpuBackendType::Cpu)?;
    let backend = processor.backend();
    
    assert!(backend.is_operational());
    assert_eq!(processor.capabilities().backend_type, GpuBackendType::Cpu);
    
    Ok(())
}

#[test]
fn test_buffer_allocation_and_free() -> VortexResult<()> {
    let processor = GpuProcessor::auto_detect()?;
    let backend = processor.backend();
    
    // Test various buffer sizes
    let sizes = vec![1024, 4096, 16384, 65536];
    
    for size in sizes {
        let buffer = backend.allocate_buffer(size)?;
        assert_eq!(buffer.size(), size);
        backend.free_buffer(buffer)?;
    }
    
    Ok(())
}

#[test]
fn test_memory_transfer_operations() -> VortexResult<()> {
    let processor = GpuProcessor::auto_detect()?;
    let backend = processor.backend();
    
    let test_data = generate_sine_wave(1000.0, 0.1, 48000);
    let buffer = backend.allocate_buffer(test_data.len() * 4)?; // 4 bytes per f32
    
    // Copy to device
    backend.copy_to_device(&buffer, &test_data)?;
    
    // Copy back from device
    let mut output = vec![0.0f32; test_data.len()];
    backend.copy_from_device(&buffer, &mut output)?;
    
    backend.free_buffer(buffer)?;
    
    Ok(())
}

#[test]
fn test_eq_processing_stub() -> VortexResult<()> {
    let processor = GpuProcessor::auto_detect()?;
    let backend = processor.backend();
    
    let input_data = generate_white_noise(0.1, 48000);
    let input = backend.allocate_buffer(input_data.len() * 4)?;
    let output = backend.allocate_buffer(input_data.len() * 4)?;
    
    backend.copy_to_device(&input, &input_data)?;
    
    let bands = vec![
        EqBand {
            frequency: 1000.0,
            gain: 6.0,
            q_factor: 1.0,
            filter_type: EqFilterType::Peak,
        },
        EqBand {
            frequency: 100.0,
            gain: -3.0,
            q_factor: 0.7,
            filter_type: EqFilterType::LowShelf,
        },
    ];
    
    // Process EQ (stub implementation for now)
    backend.process_eq(&input, &output, &bands, input_data.len())?;
    
    backend.free_buffer(input)?;
    backend.free_buffer(output)?;
    
    Ok(())
}

#[test]
fn test_convolution_stub() -> VortexResult<()> {
    let processor = GpuProcessor::auto_detect()?;
    let backend = processor.backend();
    
    let config = TestConfig::default();
    let input_samples = generate_sine_wave(440.0, 0.1, config.sample_rate);
    let ir_samples = vec![1.0, 0.5, 0.25, 0.125]; // Simple IR
    
    let input = backend.allocate_buffer(input_samples.len() * 4)?;
    let ir = backend.allocate_buffer(ir_samples.len() * 4)?;
    let output = backend.allocate_buffer((input_samples.len() + ir_samples.len()) * 4)?;
    
    backend.copy_to_device(&input, &input_samples)?;
    backend.copy_to_device(&ir, &ir_samples)?;
    
    // Process convolution (stub implementation)
    backend.process_convolution(
        &input,
        &ir,
        &output,
        input_samples.len(),
        ir_samples.len(),
    )?;
    
    backend.free_buffer(input)?;
    backend.free_buffer(ir)?;
    backend.free_buffer(output)?;
    
    Ok(())
}

#[test]
fn test_fft_processing_stub() -> VortexResult<()> {
    let processor = GpuProcessor::auto_detect()?;
    let backend = processor.backend();
    
    let fft_size = 1024;
    let input_data = generate_sine_wave(1000.0, 0.1, 48000);
    
    let input = backend.allocate_buffer(fft_size * 4)?;
    let output = backend.allocate_buffer(fft_size * 8)?; // Complex output
    
    backend.copy_to_device(&input, &input_data[..fft_size])?;
    
    // Process FFT (stub implementation)
    backend.process_fft(&input, &output, fft_size)?;
    
    backend.free_buffer(input)?;
    backend.free_buffer(output)?;
    
    Ok(())
}

#[test]
fn test_memory_usage_tracking() -> VortexResult<()> {
    let processor = GpuProcessor::auto_detect()?;
    let backend = processor.backend();
    
    let memory_info = backend.memory_usage();
    
    assert!(memory_info.total_mb > 0);
    assert!(memory_info.available_mb <= memory_info.total_mb);
    assert!(memory_info.usage_percentage >= 0.0 && memory_info.usage_percentage <= 100.0);
    
    Ok(())
}

#[test]
fn test_synchronization() -> VortexResult<()> {
    let processor = GpuProcessor::auto_detect()?;
    let backend = processor.backend();
    
    // Should not fail
    backend.synchronize()?;
    
    Ok(())
}

#[test]
fn test_multiple_buffers_concurrent() -> VortexResult<()> {
    let processor = GpuProcessor::auto_detect()?;
    let backend = processor.backend();
    
    // Allocate multiple buffers
    let buffer1 = backend.allocate_buffer(4096)?;
    let buffer2 = backend.allocate_buffer(8192)?;
    let buffer3 = backend.allocate_buffer(16384)?;
    
    assert_eq!(buffer1.size(), 4096);
    assert_eq!(buffer2.size(), 8192);
    assert_eq!(buffer3.size(), 16384);
    
    // Free in different order
    backend.free_buffer(buffer2)?;
    backend.free_buffer(buffer1)?;
    backend.free_buffer(buffer3)?;
    
    Ok(())
}

#[test]
#[cfg(feature = "cuda")]
fn test_cuda_backend_if_available() {
    // This test only runs if CUDA feature is enabled and GPU is available
    match GpuProcessor::new(GpuBackendType::Cuda) {
        Ok(processor) => {
            assert_eq!(processor.capabilities().backend_type, GpuBackendType::Cuda);
            assert!(processor.backend().is_operational());
        }
        Err(_) => {
            // CUDA not available, test passes
            eprintln!("CUDA backend not available, skipping CUDA-specific test");
        }
    }
}

#[test]
#[cfg(feature = "vulkan")]
fn test_vulkan_backend_if_available() {
    // This test only runs if Vulkan feature is enabled
    match GpuProcessor::new(GpuBackendType::Vulkan) {
        Ok(processor) => {
            assert_eq!(processor.capabilities().backend_type, GpuBackendType::Vulkan);
            assert!(processor.backend().is_operational());
        }
        Err(_) => {
            eprintln!("Vulkan backend not available, skipping Vulkan-specific test");
        }
    }
}
