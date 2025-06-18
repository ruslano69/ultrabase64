# UltraBase64

Ultra-fast Base64 encoding/decoding library with SIMD and multithreading support, written in Rust for Python.

## Features

- **Blazing Fast**: Utilizes SIMD instructions (AVX2/SSE/NEON) for maximum performance
- **Multithreaded**: Automatic multithreading for large data sets
- **Memory Efficient**: Zero-copy operations where possible
- **Cross-Platform**: Works on Linux, Windows, and macOS
- **Easy to Use**: Simple Python API

## Installation

```bash
pip install ultrabase64
```

## Usage

```python
import ultrabase64

# Basic encoding/decoding
data = b"Hello, World!"
encoded = ultrabase64.encode(data)
decoded = ultrabase64.decode(encoded)

print(f"Original: {data}")
print(f"Encoded: {encoded}")
print(f"Decoded: {decoded}")

# Encoding with specific number of threads
large_data = b"x" * 1000000
encoded_threaded = ultrabase64.encode_with_threads(large_data, threads=4)
```

## Performance

UltraBase64 automatically chooses the optimal strategy:

- **Small data** (< 256KB): Single-threaded with SIMD optimizations
- **Large data** (>= 256KB): Multi-threaded processing using all available CPU cores
- **Custom threading**: Use `encode_with_threads()` for manual control

## API Reference

### `encode(data: bytes) -> str`

Encodes bytes to Base64 string with automatic optimization.

### `decode(data: str) -> bytes`

Decodes Base64 string to bytes.

### `encode_with_threads(data: bytes, threads: int) -> str`

Encodes bytes to Base64 string using specified number of threads.

## Building from Source

Requirements:
- Python 3.8+
- Rust toolchain
- maturin

```bash
git clone https://github.com/yourusername/ultrabase64
cd ultrabase64
maturin develop --release
```

## License

This project is licensed under either of

- Apache License, Version 2.0
- MIT license

at your option.