// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod lockfree;
mod gpu;
mod validation;
mod audio;
mod fileio;
mod network;

use error::{VortexResult, AudioError, ErrorContext};
use gpu::{GpuProcessor, GpuBackendType};
use validation::{PathValidator, ParameterValidator, ResourceLimits, ResourceLimitEnforcer};

use tauri::State;
use std::sync::Arc;
use parking_lot::RwLock;

/// Application state shared across all commands
pub struct AppState {
    gpu_processor: Arc<RwLock<Option<GpuProcessor>>>,
    path_validator: Arc<PathValidator>,
    resource_limits: Arc<ResourceLimitEnforcer>,
}

impl AppState {
    fn new() -> Self {
        let limits = ResourceLimits::default();
        
        Self {
            gpu_processor: Arc::new(RwLock::new(None)),
            path_validator: Arc::new(PathValidator::new()),
            resource_limits: Arc::new(ResourceLimitEnforcer::new(limits)),
        }
    }
}

/// Initialize GPU acceleration
#[tauri::command]
async fn initialize_gpu(state: State<'_, AppState>) -> Result<String, String> {
    // Try to auto-detect best GPU backend
    match GpuProcessor::auto_detect() {
        Ok(processor) => {
            let backend_name = format!("{:?}", processor.capabilities().backend_type);
            let device_name = processor.capabilities().device_name.clone();
            
            *state.gpu_processor.write() = Some(processor);
            
            Ok(format!("Initialized {} on {}", backend_name, device_name))
        }
        Err(e) => Err(format!("Failed to initialize GPU: {}", e)),
    }
}

/// Load an audio file with validation
#[tauri::command]
async fn load_audio_file(path: String, state: State<'_, AppState>) -> Result<AudioFileInfo, String> {
    // Validate file path
    let validated_path = state.path_validator
        .validate_audio_file(&path)
        .map_err(|e| format!("Invalid file path: {}", e))?;

    // Validate file size
    let file_size = state.path_validator
        .validate_file_size(&validated_path, state.resource_limits.limits())
        .map_err(|e| format!("File size validation failed: {}", e))?;

    // TODO: Actually load and decode the file
    // For now, return metadata
    Ok(AudioFileInfo {
        path: validated_path.display().to_string(),
        size_bytes: file_size,
        duration_secs: 0.0,
        sample_rate: 0,
        channels: 0,
        format: "Unknown".to_string(),
    })
}

/// Get system status
#[tauri::command]
async fn get_system_status(state: State<'_, AppState>) -> Result<SystemStatus, String> {
    let gpu_info = if let Some(processor) = state.gpu_processor.read().as_ref() {
        let caps = processor.capabilities();
        Some(GpuInfo {
            backend: format!("{:?}", caps.backend_type),
            device_name: caps.device_name.clone(),
            compute_units: caps.compute_units,
            memory_mb: caps.max_memory_mb,
            operational: processor.backend().is_operational(),
        })
    } else {
        None
    };

    Ok(SystemStatus {
        gpu: gpu_info,
        latency_ms: 0.0, // TODO: Get actual latency
        buffer_usage_percent: 0.0, // TODO: Get actual buffer usage
    })
}

/// Validate filter parameters
#[tauri::command]
async fn validate_eq_parameters(
    frequency: f32,
    gain_db: f32,
    q_factor: f32,
    sample_rate: u32,
) -> Result<ValidatedEqParams, String> {
    let freq = ParameterValidator::validate_frequency(frequency, sample_rate)
        .map_err(|e| format!("Invalid frequency: {}", e))?;
    
    let gain = ParameterValidator::validate_gain_db(gain_db)
        .map_err(|e| format!("Invalid gain: {}", e))?;
    
    let q = ParameterValidator::validate_q_factor(q_factor)
        .map_err(|e| format!("Invalid Q factor: {}", e))?;

    Ok(ValidatedEqParams {
        frequency: freq,
        gain_db: gain,
        q_factor: q,
    })
}

// Response types for commands
#[derive(Debug, serde::Serialize)]
struct AudioFileInfo {
    path: String,
    size_bytes: u64,
    duration_secs: f64,
    sample_rate: u32,
    channels: u16,
    format: String,
}

#[derive(Debug, serde::Serialize)]
struct SystemStatus {
    gpu: Option<GpuInfo>,
    latency_ms: f64,
    buffer_usage_percent: f32,
}

#[derive(Debug, serde::Serialize)]
struct GpuInfo {
    backend: String,
    device_name: String,
    compute_units: u32,
    memory_mb: usize,
    operational: bool,
}

#[derive(Debug, serde::Serialize)]
struct ValidatedEqParams {
    frequency: f32,
    gain_db: f32,
    q_factor: f32,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            initialize_gpu,
            load_audio_file,
            get_system_status,
            validate_eq_parameters,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
