#!/usr/bin/env python3
"""
Comprehensive test suite for ultrabase64
"""

import base64
import random
import time
import sys
import threading
from concurrent.futures import ThreadPoolExecutor

try:
    import ultrabase64
except ImportError as e:
    print(f"❌ Error importing ultrabase64: {e}")
    print("💡 Make sure the module is built: maturin develop")
    sys.exit(1)

def test_multithreaded_correctness():
    """Критический тест: проверяем корректность многопоточного кодирования"""
    print("🔍 Testing multithreaded encoding correctness...")
    
    # Тестируем разные размеры, включая граничные случаи
    test_sizes = [
        3,      # Минимум для одного Base64 блока
        6,      # Два блока
        1024,   # 1KB
        3000,   # Не кратно 3
        3003,   # Кратно 3
        100_000,    # 100KB
        1_000_000,  # 1MB - порог многопоточности
        2_000_001,  # >1MB, не кратно 3
        5_000_004,  # >1MB, кратно 3
    ]
    
    for size in test_sizes:
        # Генерируем детерминированные тестовые данные
        random.seed(42)
        test_data = bytes([random.randint(0, 255) for _ in range(size)])
        
        # Эталонное кодирование
        expected = base64.b64encode(test_data).decode('utf-8')
        
        # Наше кодирование (автоматический режим)
        result_auto = ultrabase64.encode(test_data)
        
        # Наше кодирование с явно заданным количеством потоков
        result_1_thread = ultrabase64.encode_with_threads(test_data, 1)
        result_4_threads = ultrabase64.encode_with_threads(test_data, 4)
        result_8_threads = ultrabase64.encode_with_threads(test_data, 8)
        
        # Все результаты должны быть одинаковыми
        assert result_auto == expected, f"Auto mode failed for size {size}"
        assert result_1_thread == expected, f"1 thread failed for size {size}"
        assert result_4_threads == expected, f"4 threads failed for size {size}"
        assert result_8_threads == expected, f"8 threads failed for size {size}"
        
        # Проверяем обратное декодирование
        decoded = ultrabase64.decode(result_auto)
        assert decoded == test_data, f"Round trip failed for size {size}"
        
        print(f"  ✓ Size {size:>8} bytes: OK")
    
    print("✅ Multithreaded correctness tests passed")

def test_performance_scaling():
    """Тест масштабирования производительности"""
    print("📊 Testing performance scaling...")
    
    # Большие данные для тестирования многопоточности
    size = 10 * 1024 * 1024  # 10MB
    random.seed(42)
    test_data = bytes([random.randint(0, 255) for _ in range(size)])
    
    print(f"Testing with {size // (1024*1024)}MB of data...")
    
    # Тестируем с разным количеством потоков
    thread_counts = [1, 2, 4, 8]
    times = {}
    
    for threads in thread_counts:
        start_time = time.time()
        result = ultrabase64.encode_with_threads(test_data, threads)
        elapsed = time.time() - start_time
        times[threads] = elapsed
        
        # Проверяем корректность
        expected = base64.b64encode(test_data).decode('utf-8')
        assert result == expected, f"Performance test failed for {threads} threads"
        
        print(f"  {threads} threads: {elapsed:.3f}s")
    
    # Проверяем, что многопоточность дает ускорение
    if times[1] > 0:
        speedup_4 = times[1] / times[4] if times[4] > 0 else 0
        speedup_8 = times[1] / times[8] if times[8] > 0 else 0
        print(f"  Speedup 4 threads: {speedup_4:.2f}x")
        print(f"  Speedup 8 threads: {speedup_8:.2f}x")
    
    print("✅ Performance scaling test completed")

def test_concurrent_access():
    """Тест параллельного доступа из множества Python потоков"""
    print("🔄 Testing concurrent access...")
    
    def worker_encode(data, results, index):
        """Worker function для тестирования в потоке"""
        try:
            result = ultrabase64.encode(data)
            results[index] = result
        except Exception as e:
            results[index] = f"ERROR: {e}"
    
    def worker_decode(data, results, index):
        """Worker function для декодирования в потоке"""
        try:
            result = ultrabase64.decode(data)
            results[index] = result
        except Exception as e:
            results[index] = f"ERROR: {e}"
    
    # Тестовые данные
    test_data = b"Hello, concurrent world!" * 1000
    expected_encoded = base64.b64encode(test_data).decode('utf-8')
    
    # Тест параллельного кодирования
    num_workers = 10
    results = [None] * num_workers
    threads = []
    
    for i in range(num_workers):
        t = threading.Thread(target=worker_encode, args=(test_data, results, i))
        threads.append(t)
        t.start()
    
    for t in threads:
        t.join()
    
    # Проверяем результаты
    for i, result in enumerate(results):
        assert result == expected_encoded, f"Concurrent encode failed for worker {i}: {result}"
    
    # Тест параллельного декодирования
    results = [None] * num_workers
    threads = []
    
    for i in range(num_workers):
        t = threading.Thread(target=worker_decode, args=(expected_encoded, results, i))
        threads.append(t)
        t.start()
    
    for t in threads:
        t.join()
    
    # Проверяем результаты
    for i, result in enumerate(results):
        assert result == test_data, f"Concurrent decode failed for worker {i}"
    
    print("✅ Concurrent access test passed")

def test_error_handling():
    """Тест обработки ошибок"""
    print("⚠️  Testing error handling...")
    
    # Тест слишком больших данных
    max_size = ultrabase64.MAX_INPUT_SIZE
    try:
        large_data = b"x" * (max_size + 1)
        ultrabase64.encode(large_data)
        assert False, "Should have raised ValueError for large input"
    except ValueError as e:
        assert "too large" in str(e).lower()
        print(f"  ✓ Large input protection: {e}")
    
    # Тест неверного Base64
    invalid_inputs = [
        "Invalid base64!",  # Неверные символы
        "ABC",              # Неверная длина
        "AB==CD",          # Padding в неправильном месте
    ]
    
    for invalid_input in invalid_inputs:
        try:
            ultrabase64.decode(invalid_input)
            assert False, f"Should have raised error for: {invalid_input}"
        except ValueError:
            print(f"  ✓ Correctly rejected: {invalid_input}")
    
    print("✅ Error handling tests passed")

def test_library_info():
    """Тест информации о библиотеке"""
    print("ℹ️  Testing library info...")
    
    info = ultrabase64.get_info()
    print(f"  Library version: {info['version']}")
    print(f"  Multithread threshold: {info['multithread_threshold']} bytes")
    print(f"  Max threads: {info['max_threads']}")
    print(f"  Max input size: {info['max_input_size']} bytes")
    print(f"  Available CPUs: {info['available_cpus']}")
    
    # Проверяем, что константы доступны
    assert hasattr(ultrabase64, 'MULTITHREAD_THRESHOLD')
    assert hasattr(ultrabase64, 'MAX_INPUT_SIZE')
    
    print("✅ Library info test passed")

def benchmark_comparison():
    """Детальное сравнение производительности"""
    print("🏎️  Running detailed performance benchmark...")
    
    sizes = [1024, 10*1024, 100*1024, 1024*1024, 5*1024*1024]  # 1KB to 5MB
    
    print("\nSize\t\tOur Time\tStd Time\tSpeedup\tThroughput")
    print("-" * 70)
    
    for size in sizes:
        # Генерируем тестовые данные
        random.seed(42)
        data = bytes([random.randint(0, 255) for _ in range(size)])
        
        # Прогреваем
        ultrabase64.encode(data)
        base64.b64encode(data)
        
        # Наша библиотека (3 итерации для точности)
        times_ours = []
        for _ in range(3):
            start = time.time()
            ultrabase64.encode(data)
            times_ours.append(time.time() - start)
        our_time = min(times_ours)  # Берем лучший результат
        
        # Стандартная библиотека
        times_std = []
        for _ in range(3):
            start = time.time()
            base64.b64encode(data)
            times_std.append(time.time() - start)
        std_time = min(times_std)
        
        speedup = std_time / our_time if our_time > 0 else float('inf')
        throughput = size / our_time / (1024*1024) if our_time > 0 else 0  # MB/s
        
        size_str = f"{size//1024}KB" if size < 1024*1024 else f"{size//(1024*1024)}MB"
        print(f"{size_str:<12}\t{our_time:.4f}s\t{std_time:.4f}s\t{speedup:.2f}x\t{throughput:.1f} MB/s")

def main():
    """Main test function"""
    print("🧪 Running comprehensive UltraBase64 tests...")
    print(f"ultrabase64 version: {ultrabase64.__version__}")
    
    try:
        test_multithreaded_correctness()
        test_performance_scaling()
        test_concurrent_access()
        test_error_handling()
        test_library_info()
        benchmark_comparison()
        
        print("\n🎉 All tests passed successfully!")
        
    except Exception as e:
        print(f"\n❌ Test failed: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    main()