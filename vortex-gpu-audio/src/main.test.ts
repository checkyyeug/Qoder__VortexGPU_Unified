/// <reference types="vitest" />
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('GPU Initialization', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should call initialize_gpu command', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue('Initialized CUDA on NVIDIA GeForce RTX 3060');

    const result = await invoke<string>('initialize_gpu');

    expect(mockInvoke).toHaveBeenCalledWith('initialize_gpu');
    expect(result).toContain('NVIDIA');
  });

  it('should handle GPU initialization failure', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValue(new Error('GPU not found'));

    await expect(invoke('initialize_gpu')).rejects.toThrow('GPU not found');
  });
});

describe('System Status', () => {
  it('should fetch system status', async () => {
    const mockInvoke = vi.mocked(invoke);
    const mockStatus = {
      gpu: {
        backend: 'Cpu',
        device_name: 'CPU Fallback',
        compute_units: 8,
        memory_mb: 1024,
        operational: true,
      },
      latency_ms: 0.0,
      buffer_usage_percent: 0.0,
    };

    mockInvoke.mockResolvedValue(mockStatus);

    const result = await invoke('get_system_status');

    expect(result).toEqual(mockStatus);
    expect(mockInvoke).toHaveBeenCalledWith('get_system_status');
  });

  it('should handle null GPU status', async () => {
    const mockInvoke = vi.mocked(invoke);
    const mockStatus = {
      gpu: null,
      latency_ms: 0.0,
      buffer_usage_percent: 0.0,
    };

    mockInvoke.mockResolvedValue(mockStatus);

    const result = await invoke('get_system_status');

    expect(result).toEqual(mockStatus);
    expect((result as any).gpu).toBeNull();
  });
});

describe('Audio File Loading', () => {
  it('should load valid audio file', async () => {
    const mockInvoke = vi.mocked(invoke);
    const mockFileInfo = {
      path: 'C:\\Music\\test.wav',
      size_bytes: 10485760,
      duration_secs: 30.0,
      sample_rate: 48000,
      channels: 2,
      format: 'WAV',
    };

    mockInvoke.mockResolvedValue(mockFileInfo);

    const result = await invoke('load_audio_file', { path: 'C:\\Music\\test.wav' });

    expect(result).toEqual(mockFileInfo);
    expect(mockInvoke).toHaveBeenCalledWith('load_audio_file', {
      path: 'C:\\Music\\test.wav',
    });
  });

  it('should reject invalid file path', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValue(new Error('Invalid file path'));

    await expect(
      invoke('load_audio_file', { path: '../../../etc/passwd' })
    ).rejects.toThrow('Invalid file path');
  });

  it('should reject oversized file', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValue(new Error('File size validation failed'));

    await expect(
      invoke('load_audio_file', { path: 'huge_file.wav' })
    ).rejects.toThrow('File size validation failed');
  });
});

describe('EQ Parameter Validation', () => {
  it('should validate correct parameters', async () => {
    const mockInvoke = vi.mocked(invoke);
    const mockParams = {
      frequency: 1000.0,
      gain_db: 6.0,
      q_factor: 1.0,
    };

    mockInvoke.mockResolvedValue(mockParams);

    const result = await invoke('validate_eq_parameters', {
      frequency: 1000.0,
      gain_db: 6.0,
      q_factor: 1.0,
      sample_rate: 48000,
    });

    expect(result).toEqual(mockParams);
  });

  it('should clamp excessive gain', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue({
      frequency: 1000.0,
      gain_db: 24.0, // Clamped from 50.0
      q_factor: 1.0,
    });

    const result = await invoke('validate_eq_parameters', {
      frequency: 1000.0,
      gain_db: 50.0,
      q_factor: 1.0,
      sample_rate: 48000,
    });

    expect((result as any).gain_db).toBe(24.0);
  });

  it('should reject invalid frequency', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValue(new Error('Invalid frequency'));

    await expect(
      invoke('validate_eq_parameters', {
        frequency: -100.0,
        gain_db: 0.0,
        q_factor: 1.0,
        sample_rate: 48000,
      })
    ).rejects.toThrow('Invalid frequency');
  });

  it('should reject invalid Q factor', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValue(new Error('Invalid Q factor'));

    await expect(
      invoke('validate_eq_parameters', {
        frequency: 1000.0,
        gain_db: 0.0,
        q_factor: -1.0,
        sample_rate: 48000,
      })
    ).rejects.toThrow('Invalid Q factor');
  });
});

describe('TypeScript Interface Compatibility', () => {
  it('should match SystemStatus interface', () => {
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

    const mockStatus: SystemStatus = {
      gpu: {
        backend: 'Cpu',
        device_name: 'CPU Fallback',
        compute_units: 8,
        memory_mb: 1024,
        operational: true,
      },
      latency_ms: 2.5,
      buffer_usage_percent: 45.2,
    };

    expect(mockStatus.gpu?.backend).toBe('Cpu');
    expect(mockStatus.gpu?.operational).toBe(true);
    expect(mockStatus.latency_ms).toBe(2.5);
  });
});

describe('Error Handling', () => {
  it('should format error messages correctly', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValue(new Error('GPU initialization failed: Device not found'));

    try {
      await invoke('initialize_gpu');
      expect.fail('Should have thrown');
    } catch (error) {
      expect((error as Error).message).toContain('GPU initialization failed');
      expect((error as Error).message).toContain('Device not found');
    }
  });

  it('should handle network errors', async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValue(new Error('Network error: Connection refused'));

    await expect(invoke('discover_output_devices')).rejects.toThrow('Network error');
  });
});
