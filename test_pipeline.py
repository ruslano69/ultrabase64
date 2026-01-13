#!/usr/bin/env python3
"""Тест производительности конвейерной реализации."""

import os
import time
import ultrabase64

def benchmark_size(size_mb):
    """Бенчмарк для определенного размера."""
    size_bytes = size_mb * 1024 * 1024
    data = os.urandom(size_bytes)

    # Прогрев
    _ = ultrabase64.encode(data)

    # Основной тест - 3 итерации
    times = []
    for _ in range(3):
        start = time.perf_counter()
        result = ultrabase64.encode(data)
        end = time.perf_counter()
        times.append(end - start)

    # Берем лучшее время
    best_time = min(times)
    speed_mbps = size_mb / best_time

    return speed_mbps, len(result)

def main():
    print("🔬 Pipeline vs Rayon Benchmark")
    print("=" * 60)
    print(f"{'Size':<10} {'Speed (MB/s)':<15} {'Encoded Size':<15}")
    print("-" * 60)

    sizes = [1, 2, 5, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100]

    for size_mb in sizes:
        try:
            speed, encoded_size = benchmark_size(size_mb)
            print(f"{size_mb:>3} MB     {speed:>8.2f} MB/s    {encoded_size:>12,} bytes")
        except Exception as e:
            print(f"{size_mb:>3} MB     ERROR: {e}")

if __name__ == "__main__":
    main()
