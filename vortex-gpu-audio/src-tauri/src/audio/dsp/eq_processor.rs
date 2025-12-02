use crate::error::VortexError;
use crate::gpu::GpuProcessor;
use super::filters::{BiquadFilter, BiquadCoefficients};
use std::sync::Arc;
use parking_lot::RwLock;

/// EQ band configuration
#[derive(Debug, Clone)]
pub struct EqBand {
    pub frequency: f32,
    pub gain_db: f32,
    pub q: f32,
    pub enabled: bool,
}

/// 512-band parametric EQ processor
pub struct EqProcessor {
    bands: Vec<EqBand>,
    filters: Vec<BiquadFilter>,
    sample_rate: f32,
    gpu_processor: Option<Arc<RwLock<GpuProcessor>>>,
    use_gpu: bool,
}

impl EqProcessor {
    /// Create a new EQ processor with specified number of bands
    pub fn new(num_bands: usize, sample_rate: f32) -> Result<Self, VortexError> {
        let mut bands = Vec::with_capacity(num_bands);
        let mut filters = Vec::with_capacity(num_bands);
        
        // Initialize bands logarithmically distributed from 20Hz to 20kHz
        for i in 0..num_bands {
            let t = i as f32 / (num_bands - 1) as f32;
            let frequency = 20.0 * (20000.0 / 20.0).powf(t);
            
            bands.push(EqBand {
                frequency,
                gain_db: 0.0,
                q: 1.0,
                enabled: true,
            });
            
            let coeffs = BiquadCoefficients::peaking(frequency, sample_rate, 1.0, 0.0);
            filters.push(BiquadFilter::new(
                format!("EQ Band {}", i),
                coeffs,
            ));
        }
        
        Ok(Self {
            bands,
            filters,
            sample_rate,
            gpu_processor: None,
            use_gpu: false,
        })
    }
    
    /// Create a 512-band EQ processor
    pub fn new_512band(sample_rate: f32) -> Result<Self, VortexError> {
        Self::new(512, sample_rate)
    }
    
    /// Set gain for a specific band
    pub fn set_band_gain(&mut self, band_index: usize, gain_db: f32) -> Result<(), VortexError> {
        if band_index >= self.bands.len() {
            return Err(crate::error::AudioError::InvalidParameter(
                format!("Band index {} out of range", band_index)
            ).into());
        }
        
        self.bands[band_index].gain_db = gain_db;
        
        // Update filter coefficients
        let band = &self.bands[band_index];
        let coeffs = BiquadCoefficients::peaking(
            band.frequency,
            self.sample_rate,
            band.q,
            gain_db,
        );
        self.filters[band_index].set_coefficients(coeffs);
        
        Ok(())
    }
    
    /// Enable GPU acceleration
    pub fn enable_gpu(&mut self, gpu: Arc<RwLock<GpuProcessor>>) {
        self.gpu_processor = Some(gpu);
        self.use_gpu = true;
    }
    
    /// Process audio through all EQ bands
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), VortexError> {
        if self.use_gpu && self.gpu_processor.is_some() {
            // TODO: GPU processing implementation
            // For now, fall back to CPU
            self.process_cpu(input, output)
        } else {
            self.process_cpu(input, output)
        }
    }
    
    /// CPU-based processing
    fn process_cpu(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), VortexError> {
        // Initialize output with input
        output.copy_from_slice(input);
        
        let mut temp_buffer = vec![0.0f32; input.len()];
        
        // Process through each enabled band
        for (i, filter) in self.filters.iter_mut().enumerate() {
            if !self.bands[i].enabled || self.bands[i].gain_db.abs() < 0.1 {
                continue;
            }
            
            filter.process(output, &mut temp_buffer);
            output.copy_from_slice(&temp_buffer);
        }
        
        Ok(())
    }
    
    /// Get number of bands
    pub fn num_bands(&self) -> usize {
        self.bands.len()
    }
    
    /// Reset all bands to flat response (0dB)
    pub fn reset(&mut self) {
        for i in 0..self.bands.len() {
            let _ = self.set_band_gain(i, 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_eq_creation() {
        let eq = EqProcessor::new_512band(48000.0);
        assert!(eq.is_ok());
        assert_eq!(eq.unwrap().num_bands(), 512);
    }
    
    #[test]
    fn test_set_band_gain() {
        let mut eq = EqProcessor::new(10, 48000.0).unwrap();
        assert!(eq.set_band_gain(0, 6.0).is_ok());
        assert_eq!(eq.bands[0].gain_db, 6.0);
    }
    
    #[test]
    fn test_invalid_band_index() {
        let mut eq = EqProcessor::new(10, 48000.0).unwrap();
        assert!(eq.set_band_gain(100, 6.0).is_err());
    }
    
    #[test]
    fn test_process() {
        let mut eq = EqProcessor::new(10, 48000.0).unwrap();
        let input = vec![1.0; 512];
        let mut output = vec![0.0; 512];
        
        assert!(eq.process(&input, &mut output).is_ok());
    }
    
    #[test]
    fn test_reset() {
        let mut eq = EqProcessor::new(10, 48000.0).unwrap();
        eq.set_band_gain(0, 6.0).unwrap();
        eq.reset();
        
        assert_eq!(eq.bands[0].gain_db, 0.0);
    }
}
