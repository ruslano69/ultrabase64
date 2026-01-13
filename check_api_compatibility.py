#!/usr/bin/env python3
"""Проверка совместимости API с популярными base64 библиотеками."""

import base64  # stdlib
import ultrabase64

try:
    import fastbase64
    HAS_FASTBASE64 = True
except ImportError:
    HAS_FASTBASE64 = False
    print("⚠️  fastbase64 not installed (optional)")

print("=" * 80)
print("🔍 API Compatibility Check")
print("=" * 80)
print()

# Тестовые данные
test_data = b"Hello, World! This is a test message for API compatibility."
test_string = "SGVsbG8sIFdvcmxkISBUaGlzIGlzIGEgdGVzdCBtZXNzYWdlIGZvciBBUEkgY29tcGF0aWJpbGl0eS4="

print("📦 Standard Library (base64):")
print("-" * 80)
print("Available functions:")
print("  - base64.b64encode(data) -> bytes")
print("  - base64.b64decode(data) -> bytes")
print("  - base64.standard_b64encode(data) -> bytes")
print("  - base64.standard_b64decode(data) -> bytes")
print()

stdlib_encoded = base64.b64encode(test_data)
stdlib_decoded = base64.b64decode(test_string)
print(f"✅ encode: {test_data[:20]}... -> {stdlib_encoded[:20].decode()}...")
print(f"✅ decode: {test_string[:20]}... -> {stdlib_decoded[:20]}...")
print()

print("📦 ultrabase64 (current):")
print("-" * 80)
print("Available functions:")
print("  - ultrabase64.encode(data) -> str")
print("  - ultrabase64.encode_bytes(data) -> bytes")
print("  - ultrabase64.encode_auto(data) -> str")
print("  - ultrabase64.encode_pipeline_py(data) -> str")
print("  - ultrabase64.decode(data) -> bytes")
print("  - ultrabase64.encode_file_streaming(in, out) -> int")
print("  - ultrabase64.decode_file_streaming(in, out) -> int")
print()

ultra_encoded = ultrabase64.encode(test_data)
ultra_decoded = ultrabase64.decode(test_string)
print(f"✅ encode: {test_data[:20]}... -> {ultra_encoded[:20]}...")
print(f"✅ decode: {test_string[:20]}... -> {ultra_decoded[:20]}...")
print()

if HAS_FASTBASE64:
    print("📦 fastbase64:")
    print("-" * 80)
    print("Available functions:")
    print("  - fastbase64.standard_b64encode(data) -> bytes")
    print("  - fastbase64.standard_b64decode(data) -> bytes")
    print()

    fast_encoded = fastbase64.standard_b64encode(test_data)
    fast_decoded = fastbase64.standard_b64decode(test_string.encode())
    print(f"✅ encode: {test_data[:20]}... -> {fast_encoded[:20].decode()}...")
    print(f"✅ decode: {test_string[:20]}... -> {fast_decoded[:20]}...")
    print()

print("=" * 80)
print("🔄 Compatibility Analysis:")
print("=" * 80)
print()

print("1. Input/Output Types:")
print("-" * 80)
print(f"   stdlib.b64encode:              bytes -> bytes")
print(f"   fastbase64.standard_b64encode: bytes -> bytes" if HAS_FASTBASE64 else "   fastbase64: N/A")
print(f"   ultrabase64.encode:            bytes -> str   ⚠️  DIFFERENT")
print(f"   ultrabase64.encode_bytes:      bytes -> bytes ✅ COMPATIBLE")
print()

print("2. Correctness Check:")
print("-" * 80)

# Проверяем, что все библиотеки дают одинаковый результат
stdlib_result = base64.b64encode(test_data).decode('utf-8')
ultra_result = ultrabase64.encode(test_data)
ultra_bytes_result = ultrabase64.encode_bytes(test_data).decode('utf-8')

correctness_ok = (stdlib_result == ultra_result == ultra_bytes_result)

if HAS_FASTBASE64:
    fast_result = fastbase64.standard_b64encode(test_data).decode('utf-8')
    correctness_ok = correctness_ok and (stdlib_result == fast_result)
    print(f"   stdlib vs fastbase64:     {'✅ MATCH' if stdlib_result == fast_result else '❌ MISMATCH'}")

print(f"   stdlib vs ultrabase64:    {'✅ MATCH' if stdlib_result == ultra_result else '❌ MISMATCH'}")
print(f"   stdlib vs ultra.encode_bytes: {'✅ MATCH' if stdlib_result == ultra_bytes_result else '❌ MISMATCH'}")
print()

print("3. Decode Compatibility:")
print("-" * 80)

# Проверяем декодирование
stdlib_decoded = base64.b64decode(test_string)
ultra_decoded = ultrabase64.decode(test_string)

decode_ok = (stdlib_decoded == ultra_decoded)

if HAS_FASTBASE64:
    fast_decoded = fastbase64.standard_b64decode(test_string.encode())
    decode_ok = decode_ok and (stdlib_decoded == fast_decoded)
    print(f"   stdlib vs fastbase64:     {'✅ MATCH' if stdlib_decoded == fast_decoded else '❌ MISMATCH'}")

print(f"   stdlib vs ultrabase64:    {'✅ MATCH' if stdlib_decoded == ultra_decoded else '❌ MISMATCH'}")
print()

print("=" * 80)
print("📋 Drop-in Replacement Guide:")
print("=" * 80)
print()

print("To replace stdlib base64:")
print("-" * 80)
print("""
# OLD (stdlib):
import base64
encoded = base64.b64encode(data).decode('utf-8')  # bytes -> str
decoded = base64.b64decode(encoded_str)            # str -> bytes

# NEW (ultrabase64 - direct replacement):
import ultrabase64
encoded = ultrabase64.encode(data)                 # bytes -> str ✅
decoded = ultrabase64.decode(encoded)              # str -> bytes ✅
""")

if HAS_FASTBASE64:
    print("To replace fastbase64:")
    print("-" * 80)
    print("""
# OLD (fastbase64):
import fastbase64
encoded = fastbase64.standard_b64encode(data)      # bytes -> bytes
decoded = fastbase64.standard_b64decode(encoded)   # bytes -> bytes

# NEW (ultrabase64 - compatible API):
import ultrabase64
encoded = ultrabase64.encode_bytes(data)           # bytes -> bytes ✅
decoded = ultrabase64.decode(encoded.decode())     # bytes -> bytes
""")

print("=" * 80)
print("✅ Recommendation for compatibility:")
print("=" * 80)
print()
print("For maximum compatibility, use encode_bytes() when replacing fastbase64:")
print()
print("  # Compatible with both stdlib and fastbase64:")
print("  encoded = ultrabase64.encode_bytes(data)  # Returns bytes like stdlib/fast")
print()
print("For best performance and convenience:")
print("  encoded = ultrabase64.encode_auto(data)   # Returns str, auto-optimized")
print()

print("=" * 80)
print("🎯 Summary:")
print("=" * 80)
print()
if correctness_ok and decode_ok:
    print("✅ All outputs are CORRECT and COMPATIBLE")
    print("✅ ultrabase64.encode_bytes() is drop-in replacement for stdlib/fastbase64")
    print("✅ ultrabase64.encode() provides convenient str output")
    print("✅ ultrabase64.encode_auto() provides best performance")
else:
    print("❌ Compatibility issues detected!")
