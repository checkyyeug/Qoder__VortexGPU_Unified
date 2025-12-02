use crate::error::{AudioError, VortexError};
use crate::gpu::GpuProcessor;
use crate::lockfree::AudioRingBuffer;
use super::processor::AudioProcessor;
use super::filters::FilterChain;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread::{self, JoinHandle};
use parking_lot::RwLock;

/// Audio engine configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub buffer_size: usize,
    pub channels: u16,
    pub enable_gpu: bool,
    pub gpu_backend: Option<String>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            buffer_size: 512,
            channels: 2,
            enable_gpu: true,
            gpu_backend: None,
        }
    }
}

/// Audio engine error types
#[derive(Debug, thiserror::Error)]
pub enum AudioEngineError {
    #[error("Engine not initialized")]
    NotInitialized,
    #[error("Engine already running")]
    AlreadyRunning,
    #[error("Audio processing error: {0}")]
    ProcessingError(String),
    #[error("GPU initialization failed: {0}")]
    GpuInitFailed(String),
}

/// Main audio processing engine
pub struct AudioEngine {
    config: AudioConfig,
    processor: Arc<RwLock<Option<AudioProcessor>>>,
    filter_chain: Arc<RwLock<FilterChain>>,
    gpu_processor: Arc<RwLock<Option<GpuProcessor>>>,
    input_buffer: Arc<AudioRingBuffer>,
    output_buffer: Arc<AudioRingBuffer>,
    processing_thread: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl AudioEngine {
    /// Create a new audio engine with the given configuration
    pub fn new(config: AudioConfig) -> Result<Self, VortexError> {
        // Calculate buffer capacity (5 seconds of audio)
        let buffer_capacity = (config.sample_rate * 5) as usize;
        
        let input_buffer = Arc::new(AudioRingBuffer::new(
            buffer_capacity,
            config.sample_rate,
        )?);
        
        let output_buffer = Arc::new(AudioRingBuffer::new(
            buffer_capacity,
            config.sample_rate,
        )?);
        
        let filter_chain = Arc::new(RwLock::new(FilterChain::new()));
        
        Ok(Self {
            config,
            processor: Arc::new(RwLock::new(None)),
            filter_chain,
            gpu_processor: Arc::new(RwLock::new(None)),
            input_buffer,
            output_buffer,
            processing_thread: None,
            running: Arc::new(AtomicBool::new(false)),
        })
    }
    
    /// Initialize the audio engine
    pub fn initialize(&mut self) -> Result<(), VortexError> {
        // Initialize audio processor
        let processor = AudioProcessor::new(
            self.config.sample_rate,
            self.config.buffer_size,
            self.config.channels,
        )?;
        
        *self.processor.write() = Some(processor);
        
        // Initialize GPU if enabled
        if self.config.enable_gpu {
            match GpuProcessor::auto_detect() {
                Ok(gpu) => {
                    *self.gpu_processor.write() = Some(gpu);
                    log::info!("GPU acceleration enabled");
                }
                Err(e) => {
                    log::warn!("GPU initialization failed, using CPU fallback: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Start audio processing
    pub fn start_processing(&mut self) -> Result<(), VortexError> {
        if self.running.load(Ordering::Acquire) {
            return Err(AudioEngineError::AlreadyRunning.into());
        }
        
        if self.processor.read().is_none() {
            return Err(AudioEngineError::NotInitialized.into());
        }
        
        self.running.store(true, Ordering::Release);
        
        let running = Arc::clone(&self.running);
        let input_buffer = Arc::clone(&self.input_buffer);
        let output_buffer = Arc::clone(&self.output_buffer);
        let processor = Arc::clone(&self.processor);
        let filter_chain = Arc::clone(&self.filter_chain);
        let gpu_processor = Arc::clone(&self.gpu_processor);
        let buffer_size = self.config.buffer_size;
        let channels = self.config.channels as usize;
        
        // Spawn processing thread
        let handle = thread::Builder::new()
            .name("audio-processing".to_string())
            .spawn(move || {
                Self::processing_loop(
                    running,
                    input_buffer,
                    output_buffer,
                    processor,
                    filter_chain,
                    gpu_processor,
                    buffer_size,
                    channels,
                );
            })
            .map_err(|e| AudioError::ProcessingError(format!("Failed to spawn thread: {}", e)))?;
        
        self.processing_thread = Some(handle);
        
        log::info!("Audio processing started");
        Ok(())
    }
    
    /// Stop audio processing
    pub fn stop_processing(&mut self) -> Result<(), VortexError> {
        if !self.running.load(Ordering::Acquire) {
            return Ok(());
        }
        
        self.running.store(false, Ordering::Release);
        
        if let Some(handle) = self.processing_thread.take() {
            handle.join().map_err(|_| {
                AudioError::ProcessingError("Processing thread panicked".to_string())
            })?;
        }
        
        log::info!("Audio processing stopped");
        Ok(())
    }
    
    /// Add a filter to the processing chain
    pub fn add_filter(&self, filter: Box<dyn crate::audio::filters::Filter>) -> String {
        self.filter_chain.write().add_filter(filter)
    }
    
    /// Remove a filter from the processing chain
    pub fn remove_filter(&self, filter_id: &str) -> Result<(), VortexError> {
        self.filter_chain.write().remove_filter(filter_id)
            .map_err(|e| AudioError::InvalidParameter(format!("Filter removal failed: {}", e)).into())
    }
    
    /// Get current configuration
    pub fn config(&self) -> &AudioConfig {
        &self.config
    }
    
    /// Check if GPU acceleration is active
    pub fn is_gpu_enabled(&self) -> bool {
        self.gpu_processor.read().is_some()
    }
    
    /// Main processing loop (runs in dedicated thread)
    fn processing_loop(
        running: Arc<AtomicBool>,
        input_buffer: Arc<AudioRingBuffer>,
        output_buffer: Arc<AudioRingBuffer>,
        processor: Arc<RwLock<Option<AudioProcessor>>>,
        filter_chain: Arc<RwLock<FilterChain>>,
        gpu_processor: Arc<RwLock<Option<GpuProcessor>>>,
        buffer_size: usize,
        channels: usize,
    ) {
        let mut temp_input = vec![0.0f32; buffer_size * channels];
        let mut temp_output = vec![0.0f32; buffer_size * channels];
        
        while running.load(Ordering::Acquire) {
            // Read from input buffer
            let samples_read = match input_buffer.read_slice(&mut temp_input) {
                Ok(n) => n,
                Err(_) => {
                    // Buffer underrun, use silence
                    temp_input.fill(0.0);
                    buffer_size * channels
                }
            };
            
            if samples_read == 0 {
                // No data available, sleep briefly
                thread::sleep(std::time::Duration::from_micros(100));
                continue;
            }
            
            // Process audio through filter chain
            {
                let chain = filter_chain.read();
                chain.process(&temp_input, &mut temp_output);
            }
            
            // Apply GPU processing if available
            if let Some(gpu) = gpu_processor.read().as_ref() {
                // GPU processing would go here
                // For now, just copy the output
            }
            
            // Write to output buffer
            if let Err(e) = output_buffer.write_slice(&temp_output[..samples_read]) {
                log::error!("Output buffer write failed: {}", e);
            }
            
            // Update processor stats
            if let Some(proc) = processor.write().as_mut() {
                proc.update_stats(samples_read);
            }
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.stop_processing();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_engine_creation() {
        let config = AudioConfig::default();
        let engine = AudioEngine::new(config);
        assert!(engine.is_ok());
    }
    
    #[test]
    fn test_engine_initialization() {
        let config = AudioConfig::default();
        let mut engine = AudioEngine::new(config).unwrap();
        assert!(engine.initialize().is_ok());
    }
    
    #[test]
    fn test_engine_start_stop() {
        let config = AudioConfig {
            enable_gpu: false, // Disable GPU for test
            ..Default::default()
        };
        let mut engine = AudioEngine::new(config).unwrap();
        engine.initialize().unwrap();
        
        assert!(engine.start_processing().is_ok());
        assert!(engine.running.load(Ordering::Acquire));
        
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        assert!(engine.stop_processing().is_ok());
        assert!(!engine.running.load(Ordering::Acquire));
    }
    
    #[test]
    fn test_double_start_error() {
        let config = AudioConfig {
            enable_gpu: false,
            ..Default::default()
        };
        let mut engine = AudioEngine::new(config).unwrap();
        engine.initialize().unwrap();
        
        assert!(engine.start_processing().is_ok());
        assert!(engine.start_processing().is_err());
        
        engine.stop_processing().unwrap();
    }
}
