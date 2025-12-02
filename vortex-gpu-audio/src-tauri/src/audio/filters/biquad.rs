use super::filter_chain::{Filter, FilterMetadata};
use uuid::Uuid;

/// Filter types for biquad filter
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterType {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Allpass,
    Peaking,
    LowShelf,
    HighShelf,
}

/// Biquad filter coefficients
#[derive(Debug, Clone, Copy)]
pub struct BiquadCoefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl BiquadCoefficients {
    /// Calculate coefficients for a peaking EQ filter
    pub fn peaking(frequency: f32, sample_rate: f32, q: f32, gain_db: f32) -> Self {
        let omega = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * q);
        let a = 10.0_f32.powf(gain_db / 40.0);
        
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_omega;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_omega;
        let a2 = 1.0 - alpha / a;
        
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
    
    /// Calculate coefficients for a lowpass filter
    pub fn lowpass(frequency: f32, sample_rate: f32, q: f32) -> Self {
        let omega = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * q);
        
        let b0 = (1.0 - cos_omega) / 2.0;
        let b1 = 1.0 - cos_omega;
        let b2 = (1.0 - cos_omega) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_omega;
        let a2 = 1.0 - alpha;
        
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
    
    /// Calculate coefficients for a highpass filter
    pub fn highpass(frequency: f32, sample_rate: f32, q: f32) -> Self {
        let omega = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * q);
        
        let b0 = (1.0 + cos_omega) / 2.0;
        let b1 = -(1.0 + cos_omega);
        let b2 = (1.0 + cos_omega) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_omega;
        let a2 = 1.0 - alpha;
        
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

/// Biquad filter implementation
pub struct BiquadFilter {
    metadata: FilterMetadata,
    coeffs: BiquadCoefficients,
    // State variables (Direct Form I)
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadFilter {
    /// Create a new biquad filter
    pub fn new(name: String, coeffs: BiquadCoefficients) -> Self {
        Self {
            metadata: FilterMetadata {
                id: Uuid::new_v4().to_string(),
                name,
                enabled: true,
                bypass: false,
            },
            coeffs,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
    
    /// Create a peaking EQ filter
    pub fn peaking(frequency: f32, sample_rate: f32, q: f32, gain_db: f32) -> Self {
        let coeffs = BiquadCoefficients::peaking(frequency, sample_rate, q, gain_db);
        Self::new(format!("Peaking EQ {:.0}Hz", frequency), coeffs)
    }
    
    /// Update filter coefficients
    pub fn set_coefficients(&mut self, coeffs: BiquadCoefficients) {
        self.coeffs = coeffs;
    }
}

impl Filter for BiquadFilter {
    fn process(&mut self, input: &[f32], output: &mut [f32]) {
        for (i, &x) in input.iter().enumerate() {
            // Direct Form I implementation
            let y = self.coeffs.b0 * x
                + self.coeffs.b1 * self.x1
                + self.coeffs.b2 * self.x2
                - self.coeffs.a1 * self.y1
                - self.coeffs.a2 * self.y2;
            
            // Update state
            self.x2 = self.x1;
            self.x1 = x;
            self.y2 = self.y1;
            self.y1 = y;
            
            output[i] = y;
        }
    }
    
    fn metadata(&self) -> &FilterMetadata {
        &self.metadata
    }
    
    fn set_bypass(&mut self, bypass: bool) {
        self.metadata.bypass = bypass;
    }
    
    fn is_bypassed(&self) -> bool {
        self.metadata.bypass
    }
    
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
    
    fn clone_box(&self) -> Box<dyn Filter> {
        Box::new(BiquadFilter {
            metadata: self.metadata.clone(),
            coeffs: self.coeffs,
            x1: self.x1,
            x2: self.x2,
            y1: self.y1,
            y2: self.y2,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_biquad_creation() {
        let filter = BiquadFilter::peaking(1000.0, 48000.0, 1.0, 6.0);
        assert_eq!(filter.metadata().name, "Peaking EQ 1000Hz");
    }
    
    #[test]
    fn test_biquad_process() {
        let mut filter = BiquadFilter::peaking(1000.0, 48000.0, 1.0, 0.0);
        let input = vec![1.0, 0.0, 0.0, 0.0];
        let mut output = vec![0.0; 4];
        
        filter.process(&input, &mut output);
        
        // Output should have some response
        assert!(output[0].abs() > 0.0);
    }
    
    #[test]
    fn test_biquad_reset() {
        let mut filter = BiquadFilter::peaking(1000.0, 48000.0, 1.0, 6.0);
        let input = vec![1.0; 10];
        let mut output = vec![0.0; 10];
        
        filter.process(&input, &mut output);
        filter.reset();
        
        assert_eq!(filter.x1, 0.0);
        assert_eq!(filter.y1, 0.0);
    }
}
