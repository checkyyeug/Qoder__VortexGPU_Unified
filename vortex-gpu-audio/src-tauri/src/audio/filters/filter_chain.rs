use crate::error::VortexError;
use std::collections::HashMap;
use uuid::Uuid;

/// Filter metadata
#[derive(Debug, Clone)]
pub struct FilterMetadata {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub bypass: bool,
}

/// Base trait for all audio filters
pub trait Filter: Send + Sync {
    /// Process audio samples
    fn process(&mut self, input: &[f32], output: &mut [f32]);
    
    /// Get filter metadata
    fn metadata(&self) -> &FilterMetadata;
    
    /// Set bypass state
    fn set_bypass(&mut self, bypass: bool);
    
    /// Check if filter is bypassed
    fn is_bypassed(&self) -> bool;
    
    /// Reset filter state
    fn reset(&mut self);
    
    /// Clone the filter into a Box
    fn clone_box(&self) -> Box<dyn Filter>;
}

/// Chain of filters for sequential processing
pub struct FilterChain {
    filters: Vec<Box<dyn Filter>>,
    filter_map: HashMap<String, usize>,
    max_filters: usize,
}

impl FilterChain {
    /// Create a new filter chain
    pub fn new() -> Self {
        Self::with_capacity(32) // Default max 32 filters
    }
    
    /// Create a filter chain with specified capacity
    pub fn with_capacity(max_filters: usize) -> Self {
        Self {
            filters: Vec::new(),
            filter_map: HashMap::new(),
            max_filters,
        }
    }
    
    /// Add a filter to the chain
    pub fn add_filter(&mut self, filter: Box<dyn Filter>) -> String {
        let id = filter.metadata().id.clone();
        
        if self.filters.len() >= self.max_filters {
            log::warn!("Filter chain at maximum capacity, removing oldest filter");
            if let Some(removed) = self.filters.first() {
                let removed_id = removed.metadata().id.clone();
                self.filter_map.remove(&removed_id);
            }
            self.filters.remove(0);
        }
        
        let index = self.filters.len();
        self.filters.push(filter);
        self.filter_map.insert(id.clone(), index);
        
        log::info!("Added filter: {} at index {}", id, index);
        id
    }
    
    /// Remove a filter by ID
    pub fn remove_filter(&mut self, filter_id: &str) -> Result<(), String> {
        if let Some(&index) = self.filter_map.get(filter_id) {
            self.filters.remove(index);
            self.filter_map.remove(filter_id);
            
            // Update indices in map
            self.filter_map.clear();
            for (i, filter) in self.filters.iter().enumerate() {
                self.filter_map.insert(filter.metadata().id.clone(), i);
            }
            
            log::info!("Removed filter: {}", filter_id);
            Ok(())
        } else {
            Err(format!("Filter not found: {}", filter_id))
        }
    }
    
    /// Get a filter by ID
    pub fn get_filter(&self, filter_id: &str) -> Option<&Box<dyn Filter>> {
        self.filter_map.get(filter_id).and_then(|&index| self.filters.get(index))
    }
    
    /// Get a mutable filter by ID
    pub fn get_filter_mut(&mut self, filter_id: &str) -> Option<&mut Box<dyn Filter>> {
        self.filter_map.get(filter_id).copied().and_then(move |index| self.filters.get_mut(index))
    }
    
    /// Set bypass state for a specific filter
    pub fn set_filter_bypass(&mut self, filter_id: &str, bypass: bool) -> Result<(), String> {
        if let Some(filter) = self.get_filter_mut(filter_id) {
            filter.set_bypass(bypass);
            Ok(())
        } else {
            Err(format!("Filter not found: {}", filter_id))
        }
    }
    
    /// Process audio through the filter chain
    pub fn process(&self, input: &[f32], output: &mut [f32]) {
        if self.filters.is_empty() {
            // No filters, just copy input to output
            output.copy_from_slice(input);
            return;
        }
        
        // Use two buffers for ping-pong processing
        let mut buffer_a = input.to_vec();
        let mut buffer_b = vec![0.0f32; input.len()];
        
        for (i, filter) in self.filters.iter().enumerate() {
            if filter.is_bypassed() {
                continue;
            }
            
            if i % 2 == 0 {
                // Process from buffer_a to buffer_b
                unsafe {
                    let filter_mut = &mut *(filter.as_ref() as *const dyn Filter as *mut dyn Filter);
                    filter_mut.process(&buffer_a, &mut buffer_b);
                }
            } else {
                // Process from buffer_b to buffer_a
                unsafe {
                    let filter_mut = &mut *(filter.as_ref() as *const dyn Filter as *mut dyn Filter);
                    filter_mut.process(&buffer_b, &mut buffer_a);
                }
            }
        }
        
        // Copy final result to output
        let final_buffer = if self.filters.len() % 2 == 0 {
            &buffer_a
        } else {
            &buffer_b
        };
        output.copy_from_slice(final_buffer);
    }
    
    /// Get the number of filters in the chain
    pub fn len(&self) -> usize {
        self.filters.len()
    }
    
    /// Check if the chain is empty
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
    
    /// Clear all filters
    pub fn clear(&mut self) {
        self.filters.clear();
        self.filter_map.clear();
        log::info!("Filter chain cleared");
    }
    
    /// Get metadata for all filters
    pub fn list_filters(&self) -> Vec<FilterMetadata> {
        self.filters.iter().map(|f| f.metadata().clone()).collect()
    }
    
    /// Reset all filters
    pub fn reset_all(&mut self) {
        for filter in &mut self.filters {
            unsafe {
                let filter_mut = &mut **(filter as *mut Box<dyn Filter>);
                filter_mut.reset();
            }
        }
    }
}

impl Default for FilterChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Mock filter for testing
    struct MockFilter {
        metadata: FilterMetadata,
        gain: f32,
    }
    
    impl MockFilter {
        fn new(name: &str, gain: f32) -> Self {
            Self {
                metadata: FilterMetadata {
                    id: Uuid::new_v4().to_string(),
                    name: name.to_string(),
                    enabled: true,
                    bypass: false,
                },
                gain,
            }
        }
    }
    
    impl Filter for MockFilter {
        fn process(&mut self, input: &[f32], output: &mut [f32]) {
            for (i, &sample) in input.iter().enumerate() {
                output[i] = sample * self.gain;
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
            // Nothing to reset for this simple filter
        }
        
        fn clone_box(&self) -> Box<dyn Filter> {
            Box::new(MockFilter {
                metadata: self.metadata.clone(),
                gain: self.gain,
            })
        }
    }
    
    #[test]
    fn test_empty_chain() {
        let chain = FilterChain::new();
        assert_eq!(chain.len(), 0);
        assert!(chain.is_empty());
    }
    
    #[test]
    fn test_add_filter() {
        let mut chain = FilterChain::new();
        let filter = Box::new(MockFilter::new("Test", 1.0));
        let id = chain.add_filter(filter);
        
        assert_eq!(chain.len(), 1);
        assert!(chain.get_filter(&id).is_some());
    }
    
    #[test]
    fn test_remove_filter() {
        let mut chain = FilterChain::new();
        let filter = Box::new(MockFilter::new("Test", 1.0));
        let id = chain.add_filter(filter);
        
        assert!(chain.remove_filter(&id).is_ok());
        assert_eq!(chain.len(), 0);
        assert!(chain.get_filter(&id).is_none());
    }
    
    #[test]
    fn test_process_single_filter() {
        let mut chain = FilterChain::new();
        let filter = Box::new(MockFilter::new("Gain", 2.0));
        chain.add_filter(filter);
        
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = vec![0.0; 4];
        
        chain.process(&input, &mut output);
        
        assert_eq!(output, vec![2.0, 4.0, 6.0, 8.0]);
    }
    
    #[test]
    fn test_process_multiple_filters() {
        let mut chain = FilterChain::new();
        chain.add_filter(Box::new(MockFilter::new("Gain1", 2.0)));
        chain.add_filter(Box::new(MockFilter::new("Gain2", 3.0)));
        
        let input = vec![1.0, 2.0];
        let mut output = vec![0.0; 2];
        
        chain.process(&input, &mut output);
        
        // 1.0 * 2.0 * 3.0 = 6.0
        // 2.0 * 2.0 * 3.0 = 12.0
        assert_eq!(output, vec![6.0, 12.0]);
    }
    
    #[test]
    fn test_bypass_filter() {
        let mut chain = FilterChain::new();
        let filter_id = chain.add_filter(Box::new(MockFilter::new("Gain", 2.0)));
        
        chain.set_filter_bypass(&filter_id, true).unwrap();
        
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = vec![0.0; 4];
        
        chain.process(&input, &mut output);
        
        // Should be unchanged (bypassed)
        assert_eq!(output, input);
    }
    
    #[test]
    fn test_max_capacity() {
        let mut chain = FilterChain::with_capacity(2);
        
        let id1 = chain.add_filter(Box::new(MockFilter::new("Filter1", 1.0)));
        let id2 = chain.add_filter(Box::new(MockFilter::new("Filter2", 1.0)));
        let _id3 = chain.add_filter(Box::new(MockFilter::new("Filter3", 1.0)));
        
        // Should have removed the first filter
        assert_eq!(chain.len(), 2);
        assert!(chain.get_filter(&id1).is_none());
        assert!(chain.get_filter(&id2).is_some());
    }
    
    #[test]
    fn test_clear_chain() {
        let mut chain = FilterChain::new();
        chain.add_filter(Box::new(MockFilter::new("Filter1", 1.0)));
        chain.add_filter(Box::new(MockFilter::new("Filter2", 1.0)));
        
        chain.clear();
        
        assert_eq!(chain.len(), 0);
        assert!(chain.is_empty());
    }
    
    #[test]
    fn test_list_filters() {
        let mut chain = FilterChain::new();
        chain.add_filter(Box::new(MockFilter::new("Filter1", 1.0)));
        chain.add_filter(Box::new(MockFilter::new("Filter2", 1.0)));
        
        let list = chain.list_filters();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Filter1");
        assert_eq!(list[1].name, "Filter2");
    }
}
