# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2024-01-13

### Added

#### New Encoding Strategies
- **Pipeline Architecture**: New `encode_pipeline_py()` function using crossbeam channels + scoped threads
  - Optimal for large data (>25MB) with 11% performance improvement
  - Zero-copy borrowing through `crossbeam::scope`
  - Fixed 4-worker pool reduces cache thrashing
  - More stable performance (5% variance vs 8% in Rayon)

- **Auto Algorithm Selection**: New `encode_auto()` function (RECOMMENDED)
  - Automatically selects best algorithm based on data size
  - Highest average performance: 814 MB/s
  - Lowest variance: 0.17% (exceptional stability)
  - 100% optimal algorithm selection (within 5% of best in all cases)
  - Strategy:
    - <1MB: Single-threaded SIMD
    - 1-20MB: Rayon (optimal for L3 cache)
    - >20MB: Pipeline (stable for RAM-bound operations)

#### Documentation
- **PIPELINE_ANALYSIS.md**: Comprehensive architectural comparison of Rayon vs Pipeline vs Auto
- **STABILITY_REPORT.md**: Statistical analysis from 3 benchmark runs
- **API_COMPATIBILITY.md**: Complete compatibility guide for stdlib and fastbase64 migration
- **benchmark_go.go**: Go implementation for cache correlation hypothesis testing

#### Testing & Benchmarks
- Multiple benchmark scripts: `compare_implementations.py`, `test_auto.py`, `stability_summary.py`
- Automated API compatibility verification: `check_api_compatibility.py`

### Changed

#### Performance Improvements
- Optimized chunk size calculation (fixed 1MB chunks for L3 cache optimization)
- Fixed string concatenation with pre-allocated `String::with_capacity()`
- CPU count caching with `OnceLock` to avoid repeated system calls
- Pipeline achieves +11% performance on large files (90-100MB)

#### Dependencies
- Added `crossbeam = "0.8"` for channel-based parallelism

### Performance Benchmarks

#### Average Performance (3 runs)
- **Auto**: 814.04 MB/s, variance 0.17% ✅ (RECOMMENDED)
- Pipeline: 791.25 MB/s, variance 2.43%
- Rayon: 773.23 MB/s, variance 1.61%

#### vs Competition
- vs stdlib: 2-5x faster
- vs fastbase64: +7% average, +11% on large files (>90MB)

#### Cache Correlation Discovery
- Performance zones confirmed:
  - <20MB: In L3 cache (1400-1900 MB/s)
  - >25MB: Outside cache (430-480 MB/s, RAM-bound)
- Auto switches at 20MB boundary for optimal results

### API Compatibility

✅ **Fully compatible** with stdlib and fastbase64:
- All outputs byte-identical (verified)
- `encode_bytes()` is drop-in replacement for `fastbase64.standard_b64encode()`
- `encode()` is convenient replacement for `base64.b64encode().decode()`
- `encode_auto()` provides best performance for all data sizes

### Migration Guide

#### From stdlib
```python
# OLD
import base64
encoded = base64.b64encode(data).decode('utf-8')

# NEW (recommended)
import ultrabase64
encoded = ultrabase64.encode_auto(data)  # Single call, optimized!
```

#### From fastbase64
```python
# OLD
import fastbase64
encoded = fastbase64.standard_b64encode(data)

# NEW (drop-in replacement)
import ultrabase64
encoded = ultrabase64.encode_bytes(data)  # Compatible API
```

### Fixed

- Fixed chunk size calculation bug that caused 50% performance drop at 30MB
- Ensured chunk sizes are always divisible by 3 for correct Base64 encoding
- Fixed lifetime issues in pipeline implementation using scoped threads

### Technical Details

#### Key Architectural Decisions
1. **Fixed 1MB chunk size** for optimal L3 cache usage (1-2MB per core typical)
2. **Crossbeam scoped threads** for zero-copy borrowing (no `Arc::new(vec.clone())`)
3. **Pre-allocated output buffer** with direct offset writes (no intermediate collections)
4. **20MB switching threshold** in Auto algorithm (confirmed optimal through benchmarking)

#### Files Modified
- `src/lib.rs`: Added pipeline implementation, auto-selection, expose new functions
- `Cargo.toml`: Added crossbeam dependency

#### New Public Functions
- `encode_auto(data)` -> str (RECOMMENDED)
- `encode_pipeline_py(data)` -> str
- Module also exports: `encode()`, `encode_bytes()`, `decode()`, `encode_with_threads()`, `encode_file_streaming()`, `decode_file_streaming()`

### Stability

Verified through 3 complete benchmark cycles (45 data points):
- Auto algorithm: 0.17% variance (exceptional)
- Consistent performance across 1-100MB range
- Production-ready stability confirmed

---

## [1.0.13] - Previous Release

See git history for previous changes.
