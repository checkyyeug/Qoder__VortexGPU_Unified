import { invoke } from '@tauri-apps/api/core';
import './style.css';

interface SystemStatus {
  gpu: {
    backend: string;
    device_name: string;
    compute_units: number;
    memory_mb: number;
    operational: boolean;
  } | null;
  latency_ms: number;
  buffer_usage_percent: number;
}

let statusElement: HTMLElement | null;

async function initializeGPU() {
  try {
    const result = await invoke<string>('initialize_gpu');
    console.log('GPU initialized:', result);
    
    if (statusElement) {
      statusElement.innerHTML += `<p class="success">✓ ${result}</p>`;
    }
    
    await updateSystemStatus();
  } catch (error) {
    console.error('Failed to initialize GPU:', error);
    if (statusElement) {
      statusElement.innerHTML += `<p class="error">✗ GPU initialization failed: ${error}</p>`;
    }
  }
}

async function updateSystemStatus() {
  try {
    const status = await invoke<SystemStatus>('get_system_status');
    
    if (statusElement && status.gpu) {
      statusElement.innerHTML = `
        <div class="status-card">
          <h3>GPU Status</h3>
          <p><strong>Backend:</strong> ${status.gpu.backend}</p>
          <p><strong>Device:</strong> ${status.gpu.device_name}</p>
          <p><strong>Compute Units:</strong> ${status.gpu.compute_units}</p>
          <p><strong>Memory:</strong> ${status.gpu.memory_mb} MB</p>
          <p><strong>Status:</strong> ${status.gpu.operational ? '✓ Operational' : '✗ Not Operational'}</p>
        </div>
      `;
    }
  } catch (error) {
    console.error('Failed to get system status:', error);
  }
}

window.addEventListener('DOMContentLoaded', () => {
  statusElement = document.querySelector<HTMLElement>('#status');
  
  const initButton = document.querySelector<HTMLButtonElement>('#init-gpu');
  if (initButton) {
    initButton.addEventListener('click', initializeGPU);
  }

  // Create initial UI
  const app = document.querySelector<HTMLDivElement>('#app');
  if (app) {
    app.innerHTML = `
      <div class="container">
        <header>
          <h1>Vortex GPU Audio</h1>
          <p class="subtitle">Global Best Hi-Fi Audio Processing Engine</p>
        </header>
        
        <main>
          <section class="control-panel">
            <h2>System Control</h2>
            <button id="init-gpu" class="btn-primary">Initialize GPU Acceleration</button>
          </section>
          
          <section id="status" class="status-section">
            <p>Click "Initialize GPU Acceleration" to start</p>
          </section>
          
          <section class="info-panel">
            <h2>Phase 1 Implementation Complete</h2>
            <ul>
              <li>✓ FFI abstraction layer structure</li>
              <li>✓ Lock-free ring buffer for real-time audio</li>
              <li>✓ GPU backend trait abstraction</li>
              <li>✓ Comprehensive error handling framework</li>
              <li>✓ Input validation at trust boundaries</li>
            </ul>
          </section>
        </main>
      </div>
    `;
    
    // Re-attach event listeners after innerHTML update
    const newInitButton = document.querySelector<HTMLButtonElement>('#init-gpu');
    if (newInitButton) {
      newInitButton.addEventListener('click', initializeGPU);
    }
    statusElement = document.querySelector<HTMLElement>('#status');
  }
});
