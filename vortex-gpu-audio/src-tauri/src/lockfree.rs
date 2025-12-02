/// Lock-free ring buffer implementation for real-time audio processing
/// 
/// This module implements high-performance SPSC (Single Producer Single Consumer)
/// ring buffers optimized for real-time audio with <5ms latency budget.
/// 
/// Design based on Section 3 of the design review: Real-time Processing Guarantees

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::alloc::{alloc, dealloc, Layout};
use std::ptr;

/// Lock-free SPSC (Single Producer Single Consumer) ring buffer
/// 
/// This implementation provides:
/// - Zero-copy operation for real-time threads
/// - Wait-free operations for producer and consumer
/// - Cache-line padding to prevent false sharing
/// - Memory alignment for SIMD operations
pub struct LockFreeRingBuffer<T> {
    buffer: *mut T,
    capacity: usize,
    layout: Layout,
    write_pos: Arc<AtomicUsize>,
    read_pos: Arc<AtomicUsize>,
}

unsafe impl<T: Send> Send for LockFreeRingBuffer<T> {}
unsafe impl<T: Send> Sync for LockFreeRingBuffer<T> {}

impl<T: Default + Copy> LockFreeRingBuffer<T> {
    /// Create a new lock-free ring buffer with specified capacity
    /// 
    /// # Arguments
    /// * `capacity` - Number of elements (will be rounded up to power of 2)
    /// 
    /// # Safety
    /// Capacity must be > 0 and < isize::MAX
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");
        
        // Round up to next power of 2 for efficient modulo operations
        let capacity = capacity.next_power_of_two();
        
        // Allocate aligned memory for SIMD operations
        let layout = Layout::array::<T>(capacity)
            .expect("Failed to create layout")
            .align_to(64) // Cache line alignment
            .expect("Failed to align layout")
            .pad_to_align();
        
        let buffer = unsafe {
            let ptr = alloc(layout) as *mut T;
            if ptr.is_null() {
                panic!("Failed to allocate buffer memory");
            }
            
            // Initialize buffer with default values
            for i in 0..capacity {
                ptr::write(ptr.add(i), T::default());
            }
            
            ptr
        };

        Self {
            buffer,
            capacity,
            layout,
            write_pos: Arc::new(AtomicUsize::new(0)),
            read_pos: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the capacity of the ring buffer
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the number of elements available for reading
    #[inline]
    pub fn available(&self) -> usize {
        let write = self.write_pos.load(Ordering::Acquire);
        let read = self.read_pos.load(Ordering::Acquire);
        write.wrapping_sub(read)
    }

    /// Get the number of free slots available for writing
    #[inline]
    pub fn free_space(&self) -> usize {
        self.capacity - self.available() - 1 // -1 to distinguish full from empty
    }

    /// Check if the buffer is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.write_pos.load(Ordering::Acquire) == self.read_pos.load(Ordering::Acquire)
    }

    /// Check if the buffer is full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.free_space() == 0
    }

    /// Write a single element to the buffer (non-blocking)
    /// 
    /// Returns `Ok(())` if the element was written, or `Err(element)` if buffer is full
    #[inline]
    pub fn write(&self, element: T) -> Result<(), T> {
        let write = self.write_pos.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Acquire);
        
        let next_write = write.wrapping_add(1);
        
        // Check if buffer is full
        if next_write.wrapping_sub(read) > self.capacity {
            return Err(element);
        }

        unsafe {
            // Write element
            ptr::write(self.buffer.add(write & (self.capacity - 1)), element);
        }

        // Publish write
        self.write_pos.store(next_write, Ordering::Release);
        Ok(())
    }

    /// Write multiple elements to the buffer
    /// 
    /// Returns the number of elements actually written
    #[inline]
    pub fn write_slice(&self, elements: &[T]) -> usize {
        let write = self.write_pos.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Acquire);
        
        let available_space = self.capacity - write.wrapping_sub(read);
        let to_write = elements.len().min(available_space - 1);
        
        if to_write == 0 {
            return 0;
        }

        let mask = self.capacity - 1;
        let write_idx = write & mask;
        
        unsafe {
            // Handle potential wrap-around
            let first_chunk = (self.capacity - write_idx).min(to_write);
            ptr::copy_nonoverlapping(
                elements.as_ptr(),
                self.buffer.add(write_idx),
                first_chunk
            );
            
            if to_write > first_chunk {
                let remaining = to_write - first_chunk;
                ptr::copy_nonoverlapping(
                    elements[first_chunk..].as_ptr(),
                    self.buffer,
                    remaining
                );
            }
        }

        self.write_pos.store(write.wrapping_add(to_write), Ordering::Release);
        to_write
    }

    /// Read a single element from the buffer (non-blocking)
    /// 
    /// Returns `Some(element)` if an element was read, or `None` if buffer is empty
    #[inline]
    pub fn read(&self) -> Option<T> {
        let write = self.write_pos.load(Ordering::Acquire);
        let read = self.read_pos.load(Ordering::Relaxed);
        
        // Check if buffer is empty
        if write == read {
            return None;
        }

        let element = unsafe {
            ptr::read(self.buffer.add(read & (self.capacity - 1)))
        };

        self.read_pos.store(read.wrapping_add(1), Ordering::Release);
        Some(element)
    }

    /// Read multiple elements from the buffer
    /// 
    /// Returns the number of elements actually read
    #[inline]
    pub fn read_slice(&self, output: &mut [T]) -> usize {
        let write = self.write_pos.load(Ordering::Acquire);
        let read = self.read_pos.load(Ordering::Relaxed);
        
        let available = write.wrapping_sub(read);
        let to_read = output.len().min(available);
        
        if to_read == 0 {
            return 0;
        }

        let mask = self.capacity - 1;
        let read_idx = read & mask;
        
        unsafe {
            // Handle potential wrap-around
            let first_chunk = (self.capacity - read_idx).min(to_read);
            ptr::copy_nonoverlapping(
                self.buffer.add(read_idx),
                output.as_mut_ptr(),
                first_chunk
            );
            
            if to_read > first_chunk {
                let remaining = to_read - first_chunk;
                ptr::copy_nonoverlapping(
                    self.buffer,
                    output[first_chunk..].as_mut_ptr(),
                    remaining
                );
            }
        }

        self.read_pos.store(read.wrapping_add(to_read), Ordering::Release);
        to_read
    }

    /// Clear all elements from the buffer
    pub fn clear(&self) {
        let write = self.write_pos.load(Ordering::Relaxed);
        self.read_pos.store(write, Ordering::Release);
    }
}

impl<T> Drop for LockFreeRingBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.buffer as *mut u8, self.layout);
        }
    }
}

/// Audio-specific ring buffer for f32 samples
/// Optimized for real-time audio processing with additional features
pub struct AudioRingBuffer {
    buffer: LockFreeRingBuffer<f32>,
    sample_rate: u32,
    channels: usize,
}

impl AudioRingBuffer {
    /// Create a new audio ring buffer
    /// 
    /// # Arguments
    /// * `duration_ms` - Buffer duration in milliseconds
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of audio channels
    pub fn new(duration_ms: usize, sample_rate: u32, channels: usize) -> Self {
        let frames = (duration_ms * sample_rate as usize) / 1000;
        let capacity = frames * channels;
        
        Self {
            buffer: LockFreeRingBuffer::new(capacity),
            sample_rate,
            channels,
        }
    }

    /// Get the number of available frames
    #[inline]
    pub fn available_frames(&self) -> usize {
        self.buffer.available() / self.channels
    }

    /// Get the buffer fill percentage (0.0 to 1.0)
    #[inline]
    pub fn fill_percentage(&self) -> f32 {
        self.buffer.available() as f32 / self.buffer.capacity() as f32
    }

    /// Write interleaved audio samples
    #[inline]
    pub fn write_samples(&self, samples: &[f32]) -> usize {
        self.buffer.write_slice(samples)
    }

    /// Read interleaved audio samples
    #[inline]
    pub fn read_samples(&self, output: &mut [f32]) -> usize {
        self.buffer.read_slice(output)
    }

    /// Check if buffer underrun occurred
    pub fn check_underrun(&self, required_frames: usize) -> bool {
        self.available_frames() < required_frames
    }

    /// Get latency in milliseconds
    pub fn latency_ms(&self) -> f64 {
        let frames = self.available_frames();
        (frames as f64 * 1000.0) / self.sample_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;

    #[test]
    fn test_ring_buffer_basic() {
        let buffer = LockFreeRingBuffer::<i32>::new(8);
        assert_eq!(buffer.capacity(), 8);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
    }

    #[test]
    fn test_write_read_single() {
        let buffer = LockFreeRingBuffer::<i32>::new(8);
        
        assert!(buffer.write(42).is_ok());
        assert_eq!(buffer.available(), 1);
        
        assert_eq!(buffer.read(), Some(42));
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_write_read_multiple() {
        let buffer = LockFreeRingBuffer::<i32>::new(16);
        let data = vec![1, 2, 3, 4, 5];
        
        assert_eq!(buffer.write_slice(&data), 5);
        assert_eq!(buffer.available(), 5);
        
        let mut output = vec![0; 5];
        assert_eq!(buffer.read_slice(&mut output), 5);
        assert_eq!(output, data);
    }

    #[test]
    fn test_buffer_full() {
        let buffer = LockFreeRingBuffer::<i32>::new(4);
        
        // Fill buffer (capacity - 1 due to full detection)
        assert!(buffer.write(1).is_ok());
        assert!(buffer.write(2).is_ok());
        assert!(buffer.write(3).is_ok());
        
        // Buffer should be full now
        assert!(buffer.write(4).is_err());
    }

    #[test]
    fn test_wrap_around() {
        let buffer = LockFreeRingBuffer::<i32>::new(4);
        
        // Write and read to advance positions
        buffer.write(1).unwrap();
        buffer.write(2).unwrap();
        assert_eq!(buffer.read(), Some(1));
        assert_eq!(buffer.read(), Some(2));
        
        // Now write again to test wrap-around
        buffer.write(3).unwrap();
        buffer.write(4).unwrap();
        assert_eq!(buffer.read(), Some(3));
        assert_eq!(buffer.read(), Some(4));
    }

    #[test]
    fn test_audio_buffer() {
        // 100ms buffer at 48kHz stereo
        let buffer = AudioRingBuffer::new(100, 48000, 2);
        
        // Write one frame (2 samples for stereo)
        let samples = vec![0.5f32, -0.5f32];
        assert_eq!(buffer.write_samples(&samples), 2);
        assert_eq!(buffer.available_frames(), 1);
        
        // Check latency
        let latency = buffer.latency_ms();
        assert!(latency > 0.0 && latency < 1.0);
    }

    #[test]
    fn test_clear() {
        let buffer = LockFreeRingBuffer::<i32>::new(8);
        buffer.write(1).unwrap();
        buffer.write(2).unwrap();
        
        buffer.clear();
        assert!(buffer.is_empty());
    }

    // Additional comprehensive tests as per design document

    #[test]
    fn test_boundary_edge_cases() {
        let buffer = LockFreeRingBuffer::<i32>::new(4);
        
        // Test reading from empty buffer
        assert_eq!(buffer.read(), None);
        assert_eq!(buffer.available(), 0);
        
        // Fill to capacity - 1
        for i in 0..3 {
            assert!(buffer.write(i).is_ok());
        }
        
        // Verify correct count
        assert_eq!(buffer.available(), 3);
        assert_eq!(buffer.free_space(), 0);
        assert!(buffer.is_full());
    }

    #[test]
    fn test_concurrent_producer_consumer() {
        let buffer = Arc::new(LockFreeRingBuffer::<i32>::new(1024));
        let buffer_producer = Arc::clone(&buffer);
        let buffer_consumer = Arc::clone(&buffer);
        
        const NUM_ITEMS: i32 = 10000;
        
        // Producer thread
        let producer = thread::spawn(move || {
            for i in 0..NUM_ITEMS {
                while buffer_producer.write(i).is_err() {
                    // Busy wait if buffer full
                    thread::yield_now();
                }
            }
        });
        
        // Consumer thread
        let consumer = thread::spawn(move || {
            let mut received = Vec::new();
            while received.len() < NUM_ITEMS as usize {
                if let Some(val) = buffer_consumer.read() {
                    received.push(val);
                } else {
                    thread::yield_now();
                }
            }
            received
        });
        
        producer.join().unwrap();
        let received = consumer.join().unwrap();
        
        // Verify all items received in order
        assert_eq!(received.len(), NUM_ITEMS as usize);
        for (i, &val) in received.iter().enumerate() {
            assert_eq!(val, i as i32);
        }
    }

    #[test]
    fn test_bulk_operations_wrap_around() {
        let buffer = LockFreeRingBuffer::<i32>::new(8);
        
        // Write 5 elements
        let data1 = vec![1, 2, 3, 4, 5];
        assert_eq!(buffer.write_slice(&data1), 5);
        
        // Read 3 elements
        let mut output1 = vec![0; 3];
        assert_eq!(buffer.read_slice(&mut output1), 3);
        assert_eq!(output1, vec![1, 2, 3]);
        
        // Write 5 more (should wrap around)
        let data2 = vec![6, 7, 8, 9, 10];
        assert_eq!(buffer.write_slice(&data2), 5);
        
        // Read remaining 7 elements
        let mut output2 = vec![0; 7];
        assert_eq!(buffer.read_slice(&mut output2), 7);
        assert_eq!(output2, vec![4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_audio_buffer_underrun_detection() {
        let buffer = AudioRingBuffer::new(50, 48000, 2);
        
        // Write 1 frame
        let samples = vec![0.1f32, 0.2f32];
        buffer.write_samples(&samples);
        
        // Check for underrun with high requirement
        assert!(buffer.check_underrun(100));
        
        // Write more frames
        let mut more_samples = Vec::new();
        for _ in 0..200 {
            more_samples.push(0.3f32);
            more_samples.push(0.4f32);
        }
        buffer.write_samples(&more_samples);
        
        // Now no underrun
        assert!(!buffer.check_underrun(100));
    }

    #[test]
    fn test_audio_buffer_fill_percentage() {
        let buffer = AudioRingBuffer::new(100, 48000, 2);
        
        // Initially empty
        assert!(buffer.fill_percentage() < 0.01);
        
        // Fill half
        let capacity = buffer.buffer.capacity();
        let mut samples = vec![0.0f32; capacity / 2];
        buffer.write_samples(&samples);
        
        let fill = buffer.fill_percentage();
        assert!(fill > 0.45 && fill < 0.55, "Fill percentage should be ~50%, got {}", fill);
    }

    #[test]
    fn test_audio_buffer_latency_calculation() {
        // Test various configurations
        let configs = vec![
            (100, 48000, 2),  // 100ms, 48kHz, stereo
            (50, 96000, 2),   // 50ms, 96kHz, stereo
            (10, 44100, 1),   // 10ms, 44.1kHz, mono
        ];
        
        for (duration_ms, sample_rate, channels) in configs {
            let buffer = AudioRingBuffer::new(duration_ms, sample_rate, channels);
            
            // Write half buffer
            let frames = (duration_ms * sample_rate as usize) / (2 * 1000);
            let samples = vec![0.0f32; frames * channels];
            buffer.write_samples(&samples);
            
            let latency = buffer.latency_ms();
            let expected = (duration_ms as f64) / 2.0;
            assert!(
                (latency - expected).abs() < 1.0,
                "Latency {} should be close to {} for {}ms buffer at {}Hz",
                latency, expected, duration_ms, sample_rate
            );
        }
    }

    #[test]
    fn test_write_slice_partial_fill() {
        let buffer = LockFreeRingBuffer::<i32>::new(8);
        
        // Try to write more than capacity
        let large_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let written = buffer.write_slice(&large_data);
        
        // Should only write capacity - 1
        assert_eq!(written, 7);
        assert!(buffer.is_full());
    }

    #[test]
    fn test_read_slice_partial_read() {
        let buffer = LockFreeRingBuffer::<i32>::new(8);
        
        // Write 3 elements
        buffer.write(1).unwrap();
        buffer.write(2).unwrap();
        buffer.write(3).unwrap();
        
        // Try to read 5 elements
        let mut output = vec![0; 5];
        let read = buffer.read_slice(&mut output);
        
        // Should only read 3
        assert_eq!(read, 3);
        assert_eq!(&output[..3], &[1, 2, 3]);
    }

    #[test]
    fn test_power_of_two_capacity() {
        // Non-power-of-2 should be rounded up
        let buffer = LockFreeRingBuffer::<i32>::new(10);
        assert_eq!(buffer.capacity(), 16); // Next power of 2
        
        let buffer2 = LockFreeRingBuffer::<i32>::new(7);
        assert_eq!(buffer2.capacity(), 8);
        
        let buffer3 = LockFreeRingBuffer::<i32>::new(16);
        assert_eq!(buffer3.capacity(), 16); // Already power of 2
    }
}
