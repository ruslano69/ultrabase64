# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.3.0] - 2026-09-17

### Changed

#### Zero-copy входы
- Убраны все `to_vec()`/`to_owned()` на Python-входах (`encode`, `encode_bytes`,
  `encode_auto`, `encode_pipeline_py`, `encode_with_threads`, `decode`).
- Для detach-блоков (требуют `'static`) время жизни среза расширяется через
  `extend_lifetime` с документированным SAFETY-контрактом: Python-объект жив
  весь вызов, потоки join'ятся строго внутри блока. GIL по-прежнему отпускается
  на str-пути больших данных.

#### Выделенный пул потоков
- Новый `RAYON_POOL` на 8 потоков вместо глобального пула (32 потока на
  16-ядерной машине): меньше oversubscription и contention за аллокатор/память.
- `get_info()["rayon_threads"]` теперь отражает размер выделенного пула.

#### Многопоточный путь без аллокаций
- Чанки кодируются SIMD напрямую в непересекающиеся слайсы предвыделенного
  буфера (`Out`/`AsOut`): ноль промежуточных `String`/`Vec` и конкатенаций.
- Пропуск zeroing выходного буфера (`uninit_buffer` с доказанным покрытием).

#### `encode_bytes` в один проход
- Кодирование SIMD идёт сразу в буфер `PyBytes` через ffi (мимо
  `PyBytes::new_with`, который молча обнуляет буфер - лишний проход).
  Трафик памяти как у pybase64 (один проход) + потоки сверху.
- `encode_bytes` удерживает GIL на время вызова (зато без копий);
  str-пути больших данных GIL по-прежнему отпускают.

### Performance (Ryzen 9 5950X, очно против pybase64 1.5.0 AVX2)

| Блок | ENC ultra | ENC pybase64 | DEC ultra | DEC pybase64 |
|------|-----------|--------------|-----------|--------------|
| 128KB | 2874 MB/s (0.99x) | 2900 | **4883 (1.43x)** | 3388 |
| 1MB | 5184 (0.98x) | 5299 | **2943 (1.10x)** | 2667 |
| 10MB | **9223 (1.58x)** | 5827 | **2627 (1.08x)** | 2437 |
| 20MB | **9072 (1.58x)** | 5758 | **3154 (1.18x)** | 2668 |

Итого: 6 из 8 дисциплин за ultrabase64, 2 - паритет в пределах шума.
Decode выигрывает везде; encode - на больших блоках за счёт многопоточности.

No API changes - fully compatible with v1.2.x.

## [1.2.0] - 2026-09-17

### Changed

#### SIMD engine: `base64` -> `base64-simd`
- Replaced the table-based `base64` crate with SIMD-accelerated `base64-simd` 0.8
  (runtime AVX2/SSE/NEON detection) on all paths: single-threaded, Rayon chunks
  (`STANDARD_NO_PAD`), pipeline workers, streaming encode/decode
- Removed the `base64` dependency and the `OnceLock` no-pad engine
  (`base64-simd` is stateless)
- `get_info()` now reports `"engine": "base64-simd"`

### Performance (Ryzen 9 5950X, vs v1.1.1)

- 128KB `encode`: ~1.0 GB/s -> **~4.4 GB/s**
- 128KB `encode_bytes`: ~0.8 GB/s -> **~7.4 GB/s**
- 1MB: ~1.2 GB/s -> **~1.9 GB/s**, 10MB: ~1.3 GB/s -> **~1.65 GB/s**
- Auto-selection matrix average: ~1285 MB/s -> **~1410 MB/s**
- Gap to pybase64 (AVX2) on 128KB closed from ~5-20x to ~1.8-3x

No API changes - fully compatible with v1.1.x.

## [1.1.1] - 2026-09-17

### Changed

#### Dependencies
- `pyo3` 0.21 -> 0.29.2 (migration to `Bound` API, `Python::detach`, `PyModule`)
- `base64` 0.21 -> 0.23.1
- `rayon` 1.8 -> 1.12
- `num_cpus` 1.16 -> 1.17
- `crossbeam` 0.8 -> 0.8.4
- `rust-version` 1.70 -> 1.83

### Fixed

- Fixed type inference error (`E0282`) in `encode_bytes` with the new PyO3 `detach` API
- Replaced manual `div_ceil` with `usize::div_ceil` (clippy clean on Rust 1.98)
- Reformatted with current `rustfmt`

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
