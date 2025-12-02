use crate::error::VortexError;

/// Resampler quality presets
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResamplerQuality {
    Draft,    // 16 taps, 60dB
    Standard, // 64 taps, 96dB
    High,     // 256 taps, 120dB
    Maximum,  // 1024 taps, 150dB
}

impl ResamplerQuality {
    pub fn filter_length(&self) -> usize {
        match self {
            ResamplerQuality::Draft => 16,
            ResamplerQuality::Standard => 64,
            ResamplerQuality::High => 256,
            ResamplerQuality::Maximum => 1024,
        }
    }
}

/// Polyphase FIR resampler
pub struct Resampler {
    input_rate: u32,
    output_rate: u32,
    quality: ResamplerQuality,
    ratio: f64,
    // Filter state
    buffer: Vec<f32>,
    position: f64,
}

impl Resampler {
    /// Create a new resampler
    pub fn new(input_rate: u32, output_rate: u32, quality: ResamplerQuality) -> Result<Self, VortexError> {
        if input_rate == 0 || output_rate == 0 {
            return Err(crate::error::AudioError::InvalidParameter(
                "Sample rates must be > 0".to_string()
            ).into());
        }
        
        let ratio = output_rate as f64 / input_rate as f64;
        let filter_length = quality.filter_length();
        
        Ok(Self {
            input_rate,
            output_rate,
            quality,
            ratio,
            buffer: vec![0.0; filter_length],
            position: 0.0,
        })
    }
    
    /// Process audio with resampling
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, VortexError> {
        let mut output_count = 0;
        let mut input_index = 0;
        
        while input_index < input.len() && output_count < output.len() {
            // Simplified linear interpolation (full version would use polyphase FIR)
            let index_floor = self.position.floor() as usize;
            let fraction = self.position - self.position.floor();
            
            if index_floor + 1 < input.len() {
                let sample1 = input[index_floor];
                let sample2 = input[index_floor + 1];
                output[output_count] = sample1 + (sample2 - sample1) * fraction as f32;
                output_count += 1;
            }
            
            self.position += 1.0 / self.ratio;
            
            if self.position >= input.len() as f64 {
                break;
            }
        }
        
        // Reset position for next block
        self.position -= input.len() as f64;
        if self.position < 0.0 {
            self.position = 0.0;
        }
        
        Ok(output_count)
    }
    
    /// Reset resampler state
    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.position = 0.0;
    }
    
    /// Get resampling ratio
    pub fn ratio(&self) -> f64 {
        self.ratio
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_resampler_creation() {
        let resampler = Resampler::new(44100, 48000, ResamplerQuality::Standard);
        assert!(resampler.is_ok());
    }
    
    #[test]
    fn test_invalid_rates() {
        let resampler = Resampler::new(0, 48000, ResamplerQuality::Standard);
        assert!(resampler.is_err());
    }
    
    #[test]
    fn test_ratio_calculation() {
        let resampler = Resampler::new(44100, 48000, ResamplerQuality::Standard).unwrap();
        let expected_ratio = 48000.0 / 44100.0;
        assert!((resampler.ratio() - expected_ratio).abs() < 0.0001);
    }
    
    #[test]
    fn test_basic_resampling() {
        let mut resampler = Resampler::new(44100, 48000, ResamplerQuality::Standard).unwrap();
        let input = vec![1.0; 1024];
        let mut output = vec![0.0; 2048];
        
        let result = resampler.process(&input, &mut output);
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
    }
    
    #[test]
    fn test_quality_levels() {
        assert_eq!(ResamplerQuality::Draft.filter_length(), 16);
        assert_eq!(ResamplerQuality::Standard.filter_length(), 64);
        assert_eq!(ResamplerQuality::High.filter_length(), 256);
        assert_eq!(ResamplerQuality::Maximum.filter_length(), 1024);
    }
}
