/// Performance benchmarks for Vortex GPU Audio
/// 
/// Run with: cargo bench
/// 
/// These benchmarks measure:
/// - Lock-free ring buffer throughput
/// - Audio processing latency
/// - GPU memory transfer speeds
/// - Filter processing performance

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use vortex_gpu_audio::lockfree::{LockFreeRingBuffer, AudioRingBuffer};
use vortex_gpu_audio::gpu::{GpuProcessor, EqBand, EqFilterType};
use vortex_gpu_audio::audio::{
    AudioEngine, AudioConfig, AudioMemoryPool, PoolTier,
    dsp::{EqProcessor, Convolver, Resampler, ResamplerQuality},
    filters::{FilterChain, BiquadFilter},
};
use std::sync::{Arc, Mutex};

fn bench_ring_buffer_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_write");
    
    for size in [128, 512, 2048, 8192].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let buffer = LockFreeRingBuffer::<f32>::new(size * 2);
            let data = vec![0.5f32; size];
            
            b.iter(|| {
                black_box(buffer.write_slice(&data));
                buffer.clear(); // Reset for next iteration
            });
        });
    }
    
    group.finish();
}

fn bench_ring_buffer_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_read");
    
    for size in [128, 512, 2048, 8192].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let buffer = LockFreeRingBuffer::<f32>::new(size * 2);
            let data = vec![0.5f32; size];
            let mut output = vec![0.0f32; size];
            
            // Pre-fill buffer
            buffer.write_slice(&data);
            
            b.iter(|| {
                black_box(buffer.read_slice(&mut output));
                buffer.write_slice(&data); // Refill for next iteration
            });
        });
    }
    
    group.finish();
}

fn bench_ring_buffer_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_concurrent");
    
    group.bench_function("producer_consumer_512", |b| {
        use std::sync::Arc;
        use std::thread;
        
        b.iter(|| {
            let buffer = Arc::new(LockFreeRingBuffer::<f32>::new(1024));
            let buffer_producer = Arc::clone(&buffer);
            let buffer_consumer = Arc::clone(&buffer);
            
            const NUM_SAMPLES: usize = 512;
            let data = vec![0.5f32; NUM_SAMPLES];
            
            let producer = thread::spawn(move || {
                buffer_producer.write_slice(&data);
            });
            
            let consumer = thread::spawn(move || {
                let mut output = vec![0.0f32; NUM_SAMPLES];
                while buffer_consumer.read_slice(&mut output) < NUM_SAMPLES {
                    thread::yield_now();
                }
            });
            
            producer.join().unwrap();
            consumer.join().unwrap();
        });
    });
    
    group.finish();
}

fn bench_audio_ring_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_ring_buffer");
    
    group.bench_function("write_read_stereo_512", |b| {
        let buffer = AudioRingBuffer::new(100, 48000, 2);
        let samples = vec![0.5f32; 512];
        let mut output = vec![0.0f32; 512];
        
        b.iter(|| {
            black_box(buffer.write_samples(&samples));
            black_box(buffer.read_samples(&mut output));
        });
    });
    
    group.finish();
}

fn bench_gpu_buffer_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_buffer_allocation");
    
    let processor = GpuProcessor::auto_detect().unwrap();
    let backend = processor.backend();
    
    for size in [1024, 4096, 16384, 65536].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let buffer = backend.allocate_buffer(size * 4).unwrap();
                black_box(backend.free_buffer(buffer).unwrap());
            });
        });
    }
    
    group.finish();
}

fn bench_gpu_memory_transfer(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_memory_transfer");
    group.throughput(Throughput::Elements(8192));
    
    let processor = GpuProcessor::auto_detect().unwrap();
    let backend = processor.backend();
    
    group.bench_function("host_to_device_8192", |b| {
        let data = vec![0.5f32; 8192];
        let buffer = backend.allocate_buffer(8192 * 4).unwrap();
        
        b.iter(|| {
            black_box(backend.copy_to_device(&buffer, &data).unwrap());
        });
        
        backend.free_buffer(buffer).unwrap();
    });
    
    group.bench_function("device_to_host_8192", |b| {
        let data = vec![0.5f32; 8192];
        let mut output = vec![0.0f32; 8192];
        let buffer = backend.allocate_buffer(8192 * 4).unwrap();
        backend.copy_to_device(&buffer, &data).unwrap();
        
        b.iter(|| {
            black_box(backend.copy_from_device(&buffer, &mut output).unwrap());
        });
        
        backend.free_buffer(buffer).unwrap();
    });
    
    group.finish();
}

fn bench_eq_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("eq_processing");
    group.throughput(Throughput::Elements(512));
    
    let processor = GpuProcessor::auto_detect().unwrap();
    let backend = processor.backend();
    
    group.bench_function("single_band_512_samples", |b| {
        let data = vec![0.5f32; 512];
        let input = backend.allocate_buffer(512 * 4).unwrap();
        let output = backend.allocate_buffer(512 * 4).unwrap();
        backend.copy_to_device(&input, &data).unwrap();
        
        let bands = vec![EqBand {
            frequency: 1000.0,
            gain: 3.0,
            q_factor: 1.0,
            filter_type: EqFilterType::Peak,
        }];
        
        b.iter(|| {
            black_box(backend.process_eq(&input, &output, &bands, 512).unwrap());
        });
        
        backend.free_buffer(input).unwrap();
        backend.free_buffer(output).unwrap();
    });
    
    group.bench_function("multi_band_512_samples", |b| {
        let data = vec![0.5f32; 512];
        let input = backend.allocate_buffer(512 * 4).unwrap();
        let output = backend.allocate_buffer(512 * 4).unwrap();
        backend.copy_to_device(&input, &data).unwrap();
        
        let bands = vec![
            EqBand {
                frequency: 100.0,
                gain: 2.0,
                q_factor: 0.7,
                filter_type: EqFilterType::LowShelf,
            },
            EqBand {
                frequency: 1000.0,
                gain: 3.0,
                q_factor: 1.0,
                filter_type: EqFilterType::Peak,
            },
            EqBand {
                frequency: 10000.0,
                gain: -2.0,
                q_factor: 0.7,
                filter_type: EqFilterType::HighShelf,
            },
        ];
        
        b.iter(|| {
            black_box(backend.process_eq(&input, &output, &bands, 512).unwrap());
        });
        
        backend.free_buffer(input).unwrap();
        backend.free_buffer(output).unwrap();
    });
    
    group.finish();
}

fn bench_fft_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("fft_processing");
    
    let processor = GpuProcessor::auto_detect().unwrap();
    let backend = processor.backend();
    
    for fft_size in [512, 1024, 2048, 4096].iter() {
        group.throughput(Throughput::Elements(*fft_size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(fft_size), fft_size, |b, &fft_size| {
            let data = vec![0.5f32; fft_size];
            let input = backend.allocate_buffer(fft_size * 4).unwrap();
            let output = backend.allocate_buffer(fft_size * 8).unwrap(); // Complex output
            backend.copy_to_device(&input, &data).unwrap();
            
            b.iter(|| {
                black_box(backend.process_fft(&input, &output, fft_size).unwrap());
            });
            
            backend.free_buffer(input).unwrap();
            backend.free_buffer(output).unwrap();
        });
    }
    
    group.finish();
}

fn bench_convolution_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("convolution_processing");
    
    let processor = GpuProcessor::auto_detect().unwrap();
    let backend = processor.backend();
    
    group.bench_function("short_ir_512_samples", |b| {
        let input_data = vec![0.5f32; 512];
        let ir_data = vec![1.0, 0.5, 0.25, 0.125]; // 4-tap IR
        
        let input = backend.allocate_buffer(512 * 4).unwrap();
        let ir = backend.allocate_buffer(4 * 4).unwrap();
        let output = backend.allocate_buffer((512 + 4) * 4).unwrap();
        
        backend.copy_to_device(&input, &input_data).unwrap();
        backend.copy_to_device(&ir, &ir_data).unwrap();
        
        b.iter(|| {
            black_box(backend.process_convolution(&input, &ir, &output, 512, 4).unwrap());
        });
        
        backend.free_buffer(input).unwrap();
        backend.free_buffer(ir).unwrap();
        backend.free_buffer(output).unwrap();
    });
    
    group.finish();
}

// Phase 2 Benchmarks

fn bench_memory_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pool");
    
    group.bench_function("allocate_small_512", |b| {
        let pool = Arc::new(Mutex::new(AudioMemoryPool::new()));
        
        b.iter(|| {
            let buffer = AudioMemoryPool::allocate(Arc::clone(&pool), 512);
            black_box(buffer);
        });
    });
    
    group.bench_function("allocate_large_8192", |b| {
        let pool = Arc::new(Mutex::new(AudioMemoryPool::new()));
        
        b.iter(|| {
            let buffer = AudioMemoryPool::allocate(Arc::clone(&pool), 8192);
            black_box(buffer);
        });
    });
    
    group.finish();
}

fn bench_eq_processor(c: &mut Criterion) {
    let mut group = c.benchmark_group("eq_processor");
    group.throughput(Throughput::Elements(2048));
    
    group.bench_function("512_band_process_2048_samples", |b| {
        let mut eq = EqProcessor::new_512band(48000.0).unwrap();
        eq.set_band_gain(0, 3.0).unwrap();
        eq.set_band_gain(256, -2.0).unwrap();
        
        let input = vec![0.5f32; 2048];
        let mut output = vec![0.0f32; 2048];
        
        b.iter(|| {
            black_box(eq.process(&input, &mut output).unwrap());
        });
    });
    
    group.finish();
}

fn bench_filter_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_chain");
    group.throughput(Throughput::Elements(1024));
    
    group.bench_function("4_filters_1024_samples", |b| {
        let mut chain = FilterChain::new();
        chain.add_filter(Box::new(BiquadFilter::peaking(1000.0, 48000.0, 1.0, 3.0)));
        chain.add_filter(Box::new(BiquadFilter::peaking(4000.0, 48000.0, 1.0, -2.0)));
        chain.add_filter(Box::new(BiquadFilter::peaking(8000.0, 48000.0, 1.0, 1.0)));
        chain.add_filter(Box::new(BiquadFilter::peaking(12000.0, 48000.0, 1.0, -1.0)));
        
        let input = vec![0.5f32; 1024];
        let mut output = vec![0.0f32; 1024];
        
        b.iter(|| {
            chain.process(&input, &mut output);
            black_box(&output);
        });
    });
    
    group.finish();
}

fn bench_resampler(c: &mut Criterion) {
    let mut group = c.benchmark_group("resampler");
    
    group.bench_function("44.1k_to_48k_standard_1024", |b| {
        let mut resampler = Resampler::new(44100, 48000, ResamplerQuality::Standard).unwrap();
        let input = vec![0.5f32; 1024];
        let mut output = vec![0.0f32; 2048];
        
        b.iter(|| {
            black_box(resampler.process(&input, &mut output).unwrap());
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_ring_buffer_write,
    bench_ring_buffer_read,
    bench_ring_buffer_concurrent,
    bench_audio_ring_buffer,
    bench_gpu_buffer_allocation,
    bench_gpu_memory_transfer,
    bench_eq_processing,
    bench_fft_processing,
    bench_convolution_processing,
    bench_memory_pool,
    bench_eq_processor,
    bench_filter_chain,
    bench_resampler
);

criterion_main!(benches);
