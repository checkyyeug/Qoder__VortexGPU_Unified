# Testing Guide for Vortex GPU Audio

This guide provides comprehensive instructions for running and maintaining the test suite for the Vortex GPU Audio application.

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Running Tests](#running-tests)
4. [Test Organization](#test-organization)
5. [Writing Tests](#writing-tests)
6. [Performance Benchmarks](#performance-benchmarks)
7. [Code Coverage](#code-coverage)
8. [CI/CD Integration](#cicd-integration)
9. [Troubleshooting](#troubleshooting)

## Overview

The Vortex GPU Audio project uses a multi-layered testing approach:

- **Unit Tests**: Test individual components in isolation (Rust and TypeScript)
- **Integration Tests**: Test component interactions
- **Benchmarks**: Performance testing for real-time audio requirements
- **End-to-End Tests**: Complete workflow validation

### Test Coverage Targets

| Component | Target Coverage | Current Status |
|-----------|-----------------|----------------|
| Audio Processing (lockfree, error, validation) | >95% | ✓ Comprehensive tests added |
| GPU Backends | >85% | ✓ CPU fallback fully tested |
| File I/O | >80% | ⏳ Planned |
| UI Components | >70% | ✓ TypeScript tests added |
| Overall Project | >80% | 🎯 Target |

## Prerequisites

### Required Software

1. **Rust Toolchain**
   ```bash
   # Install Rust via rustup
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # Or on Windows
   # Download and run: https://rustup.rs/
   ```

2. **Node.js and npm**
   ```bash
   # Install Node.js 18 or later
   # Download from: https://nodejs.org/
   ```

3. **Platform-Specific Dependencies**

   **Windows:**
   - Visual Studio Build Tools 2019 or later
   - Windows SDK

   **macOS:**
   - Xcode Command Line Tools
   ```bash
   xcode-select --install
   ```

   **Linux (Ubuntu/Debian):**
   ```bash
   sudo apt-get update
   sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.0-dev \
     libappindicator3-dev librsvg2-dev patchelf libasound2-dev
   ```

### Optional Tools

- **Code Coverage**: `cargo install cargo-llvm-cov`
- **Security Auditing**: `cargo install cargo-audit`
- **Benchmarking**: Built-in with Criterion (included in dev-dependencies)

## Running Tests

### Quick Start

Navigate to the project root:

```bash
cd vortex-gpu-audio
```

### Rust Tests

#### Run all unit tests:
```bash
cd src-tauri
cargo test --lib
```

#### Run all tests (including integration tests):
```bash
cargo test --verbose
```

#### Run specific test:
```bash
cargo test test_ring_buffer_concurrent
```

#### Run tests with output:
```bash
cargo test -- --nocapture
```

#### Run tests for specific module:
```bash
cargo test --lib lockfree::tests
```

### TypeScript/JavaScript Tests

#### Run frontend tests:
```bash
npm test
```

#### Run tests in watch mode:
```bash
npm test -- --watch
```

#### Run tests with UI:
```bash
npm run test:ui
```

#### Run tests with coverage:
```bash
npm run test:coverage
```

### Integration Tests

Integration tests are located in `tests/integration/`:

```bash
cd src-tauri
cargo test --test gpu_processing_tests
```

## Test Organization

### Project Structure

```
vortex-gpu-audio/
├── src-tauri/
│   ├── src/
│   │   ├── error.rs           # Unit tests at end of file
│   │   ├── gpu.rs             # Unit tests at end of file
│   │   ├── lockfree.rs        # Unit tests at end of file
│   │   ├── validation.rs      # Unit tests at end of file
│   │   └── main.rs
│   ├── benches/
│   │   └── performance_benchmarks.rs
│   └── Cargo.toml
├── tests/
│   ├── common/
│   │   └── mod.rs             # Shared test utilities
│   └── integration/
│       └── gpu_processing_tests.rs
├── src/
│   └── main.test.ts           # Frontend tests
└── vitest.config.ts
```

### Test Naming Conventions

Follow the pattern: `test_<component>_<scenario>_<expected_behavior>`

Examples:
- `test_ring_buffer_write_when_full_returns_error`
- `test_gpu_backend_auto_detect_selects_best_available`
- `test_file_validator_rejects_path_traversal`

## Writing Tests

### Rust Unit Tests

Place tests at the end of each module file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_functionality() {
        let result = some_function();
        assert_eq!(result, expected_value);
    }
}
```

### Rust Integration Tests

Create files in `tests/integration/`:

```rust
use vortex_gpu_audio::gpu::GpuProcessor;

#[test]
fn test_gpu_pipeline_integration() -> Result<(), Box<dyn std::error::Error>> {
    let processor = GpuProcessor::auto_detect()?;
    // Test multi-component interaction
    Ok(())
}
```

### TypeScript Tests

Use Vitest for frontend tests:

```typescript
import { describe, it, expect, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

describe('Component', () => {
  it('should test behavior', async () => {
    const result = await invoke('command');
    expect(result).toBeDefined();
  });
});
```

### Test Utilities

Use shared utilities from `tests/common/`:

```rust
use crate::common::{generate_sine_wave, calculate_rms};

#[test]
fn test_audio_processing() {
    let samples = generate_sine_wave(1000.0, 0.1, 48000);
    let rms = calculate_rms(&samples);
    assert!(rms > 0.6 && rms < 0.8);
}
```

## Performance Benchmarks

### Running Benchmarks

```bash
cd src-tauri
cargo bench
```

### View Benchmark Results

Results are saved in `target/criterion/`:

```bash
# Open HTML report
open target/criterion/report/index.html  # macOS
xdg-open target/criterion/report/index.html  # Linux
start target\criterion\report\index.html  # Windows
```

### Available Benchmarks

- **Ring Buffer Performance**
  - Write throughput (128, 512, 2048, 8192 samples)
  - Read throughput
  - Concurrent producer-consumer

- **GPU Operations**
  - Buffer allocation/deallocation
  - Memory transfer (host-to-device, device-to-host)
  - EQ processing (single-band, multi-band)
  - FFT processing (512, 1024, 2048, 4096 points)
  - Convolution processing

### Performance Targets

| Operation | Target | Measurement |
|-----------|--------|-------------|
| Ring buffer write (512 samples) | <5µs | Per operation |
| Ring buffer read (512 samples) | <5µs | Per operation |
| GPU memory transfer (8192 samples) | <2ms | Host to device |
| EQ processing (512 samples) | <1ms | Single band |
| FFT (2048 points) | <500µs | Forward transform |

## Code Coverage

### Generate Coverage Report

#### Rust Coverage:

```bash
cd src-tauri
cargo llvm-cov --html
```

View report at `target/llvm-cov/html/index.html`

#### TypeScript Coverage:

```bash
npm run test:coverage
```

View report at `coverage/index.html`

### Coverage Targets

- **Critical components** (audio processing): >95%
- **Error handling**: >90%
- **GPU backends**: >85%
- **Overall project**: >80%

## CI/CD Integration

### GitHub Actions

The project uses GitHub Actions for automated testing. Configuration is in `.github/workflows/test.yml`.

#### On Every Commit:
- Linting (clippy, rustfmt, eslint)
- Fast unit tests (<1 minute)
- Compilation checks

#### On Pull Request:
- Full unit test suite
- Integration tests
- Code coverage analysis
- Security audit

#### Nightly:
- Full system tests
- Cross-platform builds
- Extended stress tests
- Performance benchmarks

### Running CI Locally

Simulate CI environment locally:

```bash
# Lint checks
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# Run all tests
cargo test --verbose
npm test -- --run

# Security audit
cargo audit
```

## Troubleshooting

### Common Issues

#### 1. Rust Compilation Errors

**Problem**: Tests fail to compile
```
error: linker `link.exe` not found
```

**Solution**: Install Visual Studio Build Tools (Windows) or equivalent for your platform

#### 2. Missing Dependencies

**Problem**: 
```
error: could not find native static library `gtk-3`
```

**Solution**: Install system dependencies (see Prerequisites)

#### 3. Test Timeout

**Problem**: Tests hang or timeout

**Solution**: Check for deadlocks in concurrent tests, ensure proper cleanup

#### 4. Frontend Test Errors

**Problem**: `Cannot find module 'vitest'`

**Solution**: Install dependencies
```bash
npm install
```

#### 5. Benchmark Failures

**Problem**: Benchmarks show high variance

**Solution**: Close other applications, run benchmarks multiple times

### Debug Mode

Run tests with debug output:

```bash
# Rust tests with output
cargo test -- --nocapture

# With backtrace
RUST_BACKTRACE=1 cargo test

# Frontend tests with debug
DEBUG=* npm test
```

### Performance Profiling

Profile test execution:

```bash
# Linux
perf record cargo test
perf report

# macOS
instruments -t "Time Profiler" cargo test

# Windows
# Use Visual Studio Performance Profiler
```

## Best Practices

### 1. Test Isolation

- Each test should be independent
- Use fixtures or setup/teardown for shared state
- Avoid test interdependencies

### 2. Meaningful Assertions

```rust
// Good
assert_eq!(buffer.capacity(), 1024, "Buffer capacity should match requested size");

// Avoid
assert!(buffer.capacity() > 0);
```

### 3. Test Edge Cases

- Boundary conditions (empty, full, overflow)
- Error conditions (invalid input, resource exhaustion)
- Concurrent access patterns

### 4. Performance Tests

- Use realistic data sizes
- Test under load conditions
- Measure worst-case scenarios, not averages

### 5. Documentation

- Document complex test scenarios
- Explain non-obvious assertions
- Add references to design documents

## Continuous Improvement

### Adding New Tests

When adding features:

1. Write tests first (TDD approach recommended)
2. Ensure tests cover happy path and error cases
3. Add integration tests for component interactions
4. Update this documentation if needed

### Test Maintenance

- Review flaky tests monthly
- Update tests when requirements change
- Remove obsolete tests
- Refactor duplicate test code

### Metrics Tracking

Monitor test suite health:

- Test execution time (keep under targets)
- Coverage percentage (maintain >80%)
- Flaky test rate (aim for 0%)
- Test-to-code ratio (aim for 1:2 to 1:3)

## Resources

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Criterion Benchmarking Guide](https://bheisler.github.io/criterion.rs/book/)
- [Vitest Documentation](https://vitest.dev/)
- [Tauri Testing Guide](https://tauri.app/v1/guides/testing/)

## Getting Help

- Check existing issues: [GitHub Issues](https://github.com/your-repo/issues)
- Review design document: `Vortex_GPU_Audio_Unified_Design.md`
- Contact team lead for complex testing scenarios

---

**Last Updated**: December 2025  
**Test Framework Versions**: Rust 1.75+, Cargo 1.75+, Node.js 18+, Vitest 1.2+
