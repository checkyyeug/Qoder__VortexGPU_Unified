use std::sync::Arc;
use parking_lot::Mutex;
use std::collections::VecDeque;

/// Memory pool tier for different buffer sizes
#[derive(Debug, Clone, Copy)]
pub enum PoolTier {
    Small,   // 512 samples
    Medium,  // 2048 samples
    Large,   // 8192 samples
    Huge,    // 65536 samples
}

impl PoolTier {
    pub fn size(&self) -> usize {
        match self {
            PoolTier::Small => 512,
            PoolTier::Medium => 2048,
            PoolTier::Large => 8192,
            PoolTier::Huge => 65536,
        }
    }
    
    pub fn count(&self) -> usize {
        match self {
            PoolTier::Small => 128,
            PoolTier::Medium => 64,
            PoolTier::Large => 32,
            PoolTier::Huge => 8,
        }
    }
}

/// Audio buffer from pool with RAII cleanup
pub struct PooledBuffer {
    data: Vec<f32>,
    tier: PoolTier,
    pool: Arc<Mutex<AudioMemoryPool>>,
}

impl PooledBuffer {
    /// Get mutable slice of buffer data
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        &mut self.data
    }
    
    /// Get immutable slice of buffer data
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }
    
    /// Get buffer capacity
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        // Return buffer to pool
        let mut pool = self.pool.lock();
        let tier_pool = pool.get_tier_pool_mut(self.tier);
        
        // Clear buffer before returning
        self.data.fill(0.0);
        
        if tier_pool.len() < self.tier.count() {
            tier_pool.push_back(std::mem::take(&mut self.data));
        }
    }
}

/// Audio memory pool for zero-allocation processing
pub struct AudioMemoryPool {
    small_pool: VecDeque<Vec<f32>>,
    medium_pool: VecDeque<Vec<f32>>,
    large_pool: VecDeque<Vec<f32>>,
    huge_pool: VecDeque<Vec<f32>>,
    
    // Statistics
    allocations: usize,
    deallocations: usize,
    heap_fallbacks: usize,
}

impl AudioMemoryPool {
    /// Create a new memory pool with pre-allocated buffers
    pub fn new() -> Self {
        let mut pool = Self {
            small_pool: VecDeque::new(),
            medium_pool: VecDeque::new(),
            large_pool: VecDeque::new(),
            huge_pool: VecDeque::new(),
            allocations: 0,
            deallocations: 0,
            heap_fallbacks: 0,
        };
        
        pool.preallocate();
        pool
    }
    
    /// Pre-allocate all pool buffers
    fn preallocate(&mut self) {
        // Small buffers (512 samples)
        for _ in 0..PoolTier::Small.count() {
            self.small_pool.push_back(vec![0.0f32; PoolTier::Small.size()]);
        }
        
        // Medium buffers (2048 samples)
        for _ in 0..PoolTier::Medium.count() {
            self.medium_pool.push_back(vec![0.0f32; PoolTier::Medium.size()]);
        }
        
        // Large buffers (8192 samples)
        for _ in 0..PoolTier::Large.count() {
            self.large_pool.push_back(vec![0.0f32; PoolTier::Large.size()]);
        }
        
        // Huge buffers (65536 samples)
        for _ in 0..PoolTier::Huge.count() {
            self.huge_pool.push_back(vec![0.0f32; PoolTier::Huge.size()]);
        }
    }
    
    /// Get the appropriate tier pool
    fn get_tier_pool_mut(&mut self, tier: PoolTier) -> &mut VecDeque<Vec<f32>> {
        match tier {
            PoolTier::Small => &mut self.small_pool,
            PoolTier::Medium => &mut self.medium_pool,
            PoolTier::Large => &mut self.large_pool,
            PoolTier::Huge => &mut self.huge_pool,
        }
    }
    
    /// Allocate buffer from pool
    pub fn allocate(pool: Arc<Mutex<Self>>, samples: usize) -> PooledBuffer {
        // Determine appropriate tier
        let tier = if samples <= PoolTier::Small.size() {
            PoolTier::Small
        } else if samples <= PoolTier::Medium.size() {
            PoolTier::Medium
        } else if samples <= PoolTier::Large.size() {
            PoolTier::Large
        } else {
            PoolTier::Huge
        };
        
        let mut pool_guard = pool.lock();
        pool_guard.allocations += 1;
        
        let tier_pool = pool_guard.get_tier_pool_mut(tier);
        
        let data = if let Some(mut buffer) = tier_pool.pop_front() {
            buffer.resize(samples, 0.0);
            buffer
        } else {
            // Pool exhausted, allocate from heap
            pool_guard.heap_fallbacks += 1;
            log::warn!("Memory pool exhausted for tier {:?}, allocating from heap", tier);
            vec![0.0f32; samples]
        };
        
        drop(pool_guard);
        
        PooledBuffer {
            data,
            tier,
            pool: Arc::clone(&pool),
        }
    }
    
    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            allocations: self.allocations,
            deallocations: self.deallocations,
            heap_fallbacks: self.heap_fallbacks,
            small_available: self.small_pool.len(),
            medium_available: self.medium_pool.len(),
            large_available: self.large_pool.len(),
            huge_available: self.huge_pool.len(),
        }
    }
    
    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.allocations = 0;
        self.deallocations = 0;
        self.heap_fallbacks = 0;
    }
}

impl Default for AudioMemoryPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub allocations: usize,
    pub deallocations: usize,
    pub heap_fallbacks: usize,
    pub small_available: usize,
    pub medium_available: usize,
    pub large_available: usize,
    pub huge_available: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pool_creation() {
        let pool = AudioMemoryPool::new();
        let stats = pool.stats();
        
        assert_eq!(stats.small_available, PoolTier::Small.count());
        assert_eq!(stats.medium_available, PoolTier::Medium.count());
        assert_eq!(stats.large_available, PoolTier::Large.count());
        assert_eq!(stats.huge_available, PoolTier::Huge.count());
    }
    
    #[test]
    fn test_allocate_small() {
        let pool = Arc::new(Mutex::new(AudioMemoryPool::new()));
        let buffer = AudioMemoryPool::allocate(Arc::clone(&pool), 256);
        
        assert_eq!(buffer.capacity(), PoolTier::Small.size());
        
        let stats = pool.lock().stats();
        assert_eq!(stats.allocations, 1);
        assert_eq!(stats.small_available, PoolTier::Small.count() - 1);
    }
    
    #[test]
    fn test_buffer_return_on_drop() {
        let pool = Arc::new(Mutex::new(AudioMemoryPool::new()));
        
        {
            let _buffer = AudioMemoryPool::allocate(Arc::clone(&pool), 256);
            let stats = pool.lock().stats();
            assert_eq!(stats.small_available, PoolTier::Small.count() - 1);
        }
        
        // Buffer should be returned
        let stats = pool.lock().stats();
        assert_eq!(stats.small_available, PoolTier::Small.count());
    }
    
    #[test]
    fn test_tier_selection() {
        let pool = Arc::new(Mutex::new(AudioMemoryPool::new()));
        
        let small = AudioMemoryPool::allocate(Arc::clone(&pool), 512);
        let medium = AudioMemoryPool::allocate(Arc::clone(&pool), 2048);
        let large = AudioMemoryPool::allocate(Arc::clone(&pool), 8192);
        let huge = AudioMemoryPool::allocate(Arc::clone(&pool), 65536);
        
        assert_eq!(small.capacity(), PoolTier::Small.size());
        assert_eq!(medium.capacity(), PoolTier::Medium.size());
        assert_eq!(large.capacity(), PoolTier::Large.size());
        assert_eq!(huge.capacity(), PoolTier::Huge.size());
    }
    
    #[test]
    fn test_pool_exhaustion() {
        let pool = Arc::new(Mutex::new(AudioMemoryPool::new()));
        let mut buffers = Vec::new();
        
        // Allocate all small buffers
        for _ in 0..PoolTier::Small.count() + 5 {
            buffers.push(AudioMemoryPool::allocate(Arc::clone(&pool), 256));
        }
        
        let stats = pool.lock().stats();
        assert_eq!(stats.heap_fallbacks, 5); // Last 5 should fallback to heap
    }
    
    #[test]
    fn test_buffer_reuse() {
        let pool = Arc::new(Mutex::new(AudioMemoryPool::new()));
        
        for _ in 0..100 {
            let mut buffer = AudioMemoryPool::allocate(Arc::clone(&pool), 256);
            buffer.as_mut_slice().fill(1.0);
            // Buffer drops and returns to pool
        }
        
        let stats = pool.lock().stats();
        assert_eq!(stats.allocations, 100);
        assert_eq!(stats.heap_fallbacks, 0); // No heap allocations needed
    }
    
    #[test]
    fn test_buffer_cleared_on_return() {
        let pool = Arc::new(Mutex::new(AudioMemoryPool::new()));
        
        {
            let mut buffer = AudioMemoryPool::allocate(Arc::clone(&pool), 256);
            buffer.as_mut_slice().fill(1.0);
        }
        
        // Get buffer again
        let buffer = AudioMemoryPool::allocate(Arc::clone(&pool), 256);
        assert!(buffer.as_slice().iter().all(|&x| x == 0.0));
    }
}
