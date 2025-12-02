use vortex_gpu_audio::audio::{AudioEngine, AudioConfig};
use vortex_gpu_audio::audio::filters::{FilterChain, BiquadFilter};
use vortex_gpu_audio::audio::dsp::{EqProcessor, DsdProcessor, Convolver, Resampler, ResamplerQuality, DsdRate};

#[test]
fn test_audio_engine_initialization() -> Result<(), Box<dyn std::error::Error>> {
    let config = AudioConfig {
        sample_rate: 48000,
        buffer_size: 512,
        channels: 2,
        enable_gpu: false,
        gpu_backend: None,
    };
    
    let mut engine = AudioEngine::new(config)?;
    engine.initialize()?;
    
    Ok(())
}

#[test]
fn test_audio_engine_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let config = AudioConfig {
        sample_rate: 48000,
        buffer_size: 512,
        channels: 2,
        enable_gpu: false,
        gpu_backend: None,
    };
    
    let mut engine = AudioEngine::new(config)?;
    engine.initialize()?;
    engine.start_processing()?;
    
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    engine.stop_processing()?;
    
    Ok(())
}

#[test]
fn test_filter_chain_integration() -> Result<(), Box<dyn std::error::Error>> {
    let mut chain = FilterChain::new();
    
    // Add multiple filters
    let filter1 = Box::new(BiquadFilter::peaking(1000.0, 48000.0, 1.0, 6.0));
    let filter2 = Box::new(BiquadFilter::peaking(4000.0, 48000.0, 1.0, -3.0));
    
    let id1 = chain.add_filter(filter1);
    let id2 = chain.add_filter(filter2);
    
    assert_eq!(chain.len(), 2);
    
    // Process audio
    let input = vec![1.0; 512];
    let mut output = vec![0.0; 512];
    
    chain.process(&input, &mut output);
    
    // Remove a filter
    chain.remove_filter(&id1)?;
    assert_eq!(chain.len(), 1);
    
    Ok(())
}

#[test]
fn test_eq_processor_integration() -> Result<(), Box<dyn std::error::Error>> {
    let mut eq = EqProcessor::new_512band(48000.0)?;
    
    // Set gains for multiple bands
    eq.set_band_gain(0, 6.0)?;
    eq.set_band_gain(100, -3.0)?;
    eq.set_band_gain(500, 3.0)?;
    
    // Process audio
    let input = vec![1.0; 2048];
    let mut output = vec![0.0; 2048];
    
    eq.process(&input, &mut output)?;
    
    // Reset and verify
    eq.reset();
    assert_eq!(eq.num_bands(), 512);
    
    Ok(())
}

#[test]
fn test_dsp_pipeline_complete() -> Result<(), Box<dyn std::error::Error>> {
    // Create complete DSP pipeline
    let sample_rate = 48000.0;
    
    // 1. Resampler (44.1kHz to 48kHz)
    let mut resampler = Resampler::new(44100, 48000, ResamplerQuality::Standard)?;
    
    // 2. EQ Processor
    let mut eq = EqProcessor::new(10, sample_rate)?;
    eq.set_band_gain(0, 3.0)?;
    eq.set_band_gain(5, -6.0)?;
    
    // 3. Convolver
    let ir = vec![1.0, 0.5, 0.25, 0.125]; // Simple IR
    let mut convolver = Convolver::new(ir, 512)?;
    
    // Process through pipeline
    let input_44k = vec![1.0; 4410]; // 100ms at 44.1kHz
    let mut resampled = vec![0.0; 10000];
    
    let samples_out = resampler.process(&input_44k, &mut resampled)?;
    resampled.truncate(samples_out);
    
    let mut eq_output = vec![0.0; samples_out];
    eq.process(&resampled, &mut eq_output)?;
    
    let mut final_output = vec![0.0; samples_out];
    convolver.process(&eq_output, &mut final_output)?;
    
    assert!(final_output.len() > 0);
    
    Ok(())
}

#[test]
fn test_dsd_processing_integration() -> Result<(), Box<dyn std::error::Error>> {
    let mut processor = DsdProcessor::new(DsdRate::Dsd64, 44100)?;
    
    // Create DSD test data (all ones)
    let dsd_input = vec![0xFF; 1024];
    let mut pcm_output = vec![0.0; 1024];
    
    let samples = processor.process(&dsd_input, &mut pcm_output)?;
    
    assert!(samples > 0);
    assert!(samples <= pcm_output.len());
    
    processor.reset();
    
    Ok(())
}

#[test]
fn test_convolution_with_large_ir() -> Result<(), Box<dyn std::error::Error>> {
    // Test with larger IR (simulating room reverb)
    let ir_length = 16384; // 16K samples
    let mut ir = vec![0.0; ir_length];
    
    // Create simple decay envelope
    for (i, sample) in ir.iter_mut().enumerate() {
        *sample = (-i as f32 / 1000.0).exp();
    }
    
    let mut convolver = Convolver::new(ir, 2048)?;
    
    let input = vec![1.0, 0.0, 0.0, 0.0]; // Impulse
    let mut output = vec![0.0; 4];
    
    convolver.process(&input, &mut output)?;
    
    // First sample should be approximately 1.0 (start of IR)
    assert!((output[0] - 1.0).abs() < 0.1);
    
    Ok(())
}

#[test]
fn test_resampler_quality_comparison() -> Result<(), Box<dyn std::error::Error>> {
    let input = vec![1.0; 1000];
    
    for quality in [ResamplerQuality::Draft, ResamplerQuality::Standard, ResamplerQuality::High] {
        let mut resampler = Resampler::new(44100, 48000, quality)?;
        let mut output = vec![0.0; 2000];
        
        let samples = resampler.process(&input, &mut output)?;
        assert!(samples > 0);
        
        println!("Quality {:?}: {} samples out", quality, samples);
    }
    
    Ok(())
}

#[test]
fn test_audio_engine_with_filters() -> Result<(), Box<dyn std::error::Error>> {
    let config = AudioConfig::default();
    let mut engine = AudioEngine::new(config)?;
    engine.initialize()?;
    
    // Add filters to the engine
    let filter = Box::new(BiquadFilter::peaking(1000.0, 48000.0, 1.0, 6.0));
    let filter_id = engine.add_filter(filter);
    
    engine.start_processing()?;
    std::thread::sleep(std::time::Duration::from_millis(50));
    engine.stop_processing()?;
    
    // Remove filter
    engine.remove_filter(&filter_id)?;
    
    Ok(())
}
