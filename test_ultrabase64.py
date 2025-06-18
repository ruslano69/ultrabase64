#!/usr/bin/env python3
"""
Тесты для библиотеки ultrabase64
"""

import base64
import random
import time
import ultrabase64


def test_basic_encoding():
    """Тест базового кодирования/декодирования"""
    test_data = b"Hello, World!"
    
    # Наше кодирование
    encoded = ultrabase64.encode(test_data)
    decoded = ultrabase64.decode(encoded)
    
    # Стандартное кодирование для сравнения
    standard_encoded = base64.b64encode(test_data).decode('utf-8')
    
    assert encoded == standard_encoded, f"Encoding mismatch: {encoded} != {standard_encoded}"
    assert decoded == test_data, f"Decoding mismatch: {decoded} != {test_data}"
    print("✓ Basic encoding/decoding test passed")


def test_large_data():
    """Тест с большими данными"""
    # Создаем 1MB случайных данных
    large_data = bytes([random.randint(0, 255) for _ in range(1024 * 1024)])
    
    # Наше кодирование
    start_time = time.time()
    encoded = ultrabase64.encode(large_data)
    our_time = time.time() - start_time
    
    # Стандартное кодирование
    start_time = time.time()
    standard_encoded = base64.b64encode(large_data).decode('utf-8')
    standard_time = time.time() - start_time
    
    # Проверяем корректность
    decoded = ultrabase64.decode(encoded)
    assert decoded == large_data, "Large data decoding failed"
    assert encoded == standard_encoded, "Large data encoding mismatch"
    
    print(f"✓ Large data test passed")
    print(f"  Our time: {our_time:.4f}s, Standard time: {standard_time:.4f}s")
    print(f"  Speedup: {standard_time/our_time:.2f}x")


def test_threading():
    """Тест многопоточного кодирования"""
    large_data = bytes([random.randint(0, 255) for _ in range(512 * 1024)])
    
    # Тест с разным количеством потоков
    for threads in [1, 2, 4, 8]:
        start_time = time.time()
        encoded = ultrabase64.encode_with_threads(large_data, threads)
        thread_time = time.time() - start_time
        
        # Проверяем корректность
        decoded = ultrabase64.decode(encoded)
        assert decoded == large_data, f"Threading test failed for {threads} threads"
        
        print(f"✓ Threading test with {threads} threads: {thread_time:.4f}s")


def test_edge_cases():
    """Тест граничных случаев"""
    # Пустые данные
    assert ultrabase64.encode(b"") == ""
    assert ultrabase64.decode("") == b""
    
    # Один байт
    assert ultrabase64.encode(b"A") == base64.b64encode(b"A").decode('utf-8')
    
    # Данные, не кратные 3
    for i in range(1, 10):
        test_data = b"x" * i
        encoded = ultrabase64.encode(test_data)
        standard = base64.b64encode(test_data).decode('utf-8')
        assert encoded == standard, f"Edge case failed for {i} bytes"
    
    print("✓ Edge cases test passed")


def benchmark():
    """Бенчмарк производительности"""
    sizes = [1024, 10*1024, 100*1024, 1024*1024]  # 1KB, 10KB, 100KB, 1MB
    
    print("\n📊 Performance Benchmark:")
    print("Size\t\tOur Time\tStd Time\tSpeedup")
    print("-" * 50)
    
    for size in sizes:
        data = bytes([random.randint(0, 255) for _ in range(size)])
        
        # Наша библиотека
        start = time.time()
        for _ in range(10):  # 10 итераций для усреднения
            ultrabase64.encode(data)
        our_time = (time.time() - start) / 10
        
        # Стандартная библиотека
        start = time.time()
        for _ in range(10):
            base64.b64encode(data)
        std_time = (time.time() - start) / 10
        
        speedup = std_time / our_time if our_time > 0 else float('inf')
        
        size_str = f"{size//1024}KB" if size >= 1024 else f"{size}B"
        print(f"{size_str:<12}\t{our_time:.6f}s\t{std_time:.6f}s\t{speedup:.2f}x")


if __name__ == "__main__":
    print("🧪 Running UltraBase64 tests...")
    
    test_basic_encoding()
    test_large_data()
    test_threading()
    test_edge_cases()
    benchmark()
    
    print("\n🎉 All tests passed!")