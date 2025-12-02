use crate::error::VortexError;

/// Partition-based convolution processor
pub struct Convolver {
    ir: Vec<f32>,
    partition_size: usize,
    num_partitions: usize,
    // State buffers
    overlap_buffer: Vec<f32>,
}

impl Convolver {
    /// Create a new convolver with the given impulse response
    pub fn new(ir: Vec<f32>, partition_size: usize) -> Result<Self, VortexError> {
        if ir.is_empty() {
            return Err(crate::error::AudioError::InvalidParameter(
                "Impulse response cannot be empty".to_string()
            ).into());
        }
        
        if partition_size == 0 || !partition_size.is_power_of_two() {
            return Err(crate::error::AudioError::InvalidParameter(
                "Partition size must be power of 2".to_string()
            ).into());
        }
        
        let num_partitions = (ir.len() + partition_size - 1) / partition_size;
        let overlap_buffer = vec![0.0; partition_size * 2];
        
        Ok(Self {
            ir,
            partition_size,
            num_partitions,
            overlap_buffer,
        })
    }
    
    /// Process audio through convolution
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), VortexError> {
        // Simplified implementation - full version would use FFT-based convolution
        // This is a direct convolution (inefficient for large IRs)
        
        let ir_len = self.ir.len();
        
        for i in 0..output.len() {
            let mut sum = 0.0;
            
            for j in 0..ir_len.min(i + 1) {
                sum += input[i - j] * self.ir[j];
            }
            
            output[i] = sum;
        }
        
        Ok(())
    }
    
    /// Update the impulse response
    pub fn set_ir(&mut self, ir: Vec<f32>) -> Result<(), VortexError> {
        if ir.is_empty() {
            return Err(crate::error::AudioError::InvalidParameter(
                "Impulse response cannot be empty".to_string()
            ).into());
        }
        
        self.ir = ir;
        self.num_partitions = (self.ir.len() + self.partition_size - 1) / self.partition_size;
        self.reset();
        
        Ok(())
    }
    
    /// Reset processor state
    pub fn reset(&mut self) {
        self.overlap_buffer.fill(0.0);
    }
    
    /// Get IR length
    pub fn ir_length(&self) -> usize {
        self.ir.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_convolver_creation() {
        let ir = vec![1.0, 0.5, 0.25];
        let convolver = Convolver::new(ir, 512);
        assert!(convolver.is_ok());
    }
    
    #[test]
    fn test_invalid_partition_size() {
        let ir = vec![1.0];
        let convolver = Convolver::new(ir, 500); // Not power of 2
        assert!(convolver.is_err());
    }
    
    #[test]
    fn test_empty_ir() {
        let ir = vec![];
        let convolver = Convolver::new(ir, 512);
        assert!(convolver.is_err());
    }
    
    #[test]
    fn test_basic_convolution() {
        let ir = vec![1.0, 0.5];
        let mut convolver = Convolver::new(ir, 512).unwrap();
        
        let input = vec![1.0, 0.0, 0.0, 0.0];
        let mut output = vec![0.0; 4];
        
        assert!(convolver.process(&input, &mut output).is_ok());
        // Impulse response: should get [1.0, 0.5, 0.0, 0.0]
        assert_eq!(output[0], 1.0);
        assert_eq!(output[1], 0.5);
    }
}
