#!/usr/bin/env python3
"""Сравнение производительности Rayon vs Pipeline реализаций."""

import os
import time
import ultrabase64

def benchmark_implementation(func_name, data):
    """Бенчмарк конкретной реализации."""
    func = getattr(ultrabase64, func_name)

    # Прогрев (1 раз)
    _ = func(data)

    # Основной тест - 3 итерации
    times = []
    for _ in range(3):
        start = time.perf_counter()
        result = func(data)
        end = time.perf_counter()
        times.append(end - start)

    # Берем лучшее время
    best_time = min(times)
    return best_time, len(result)

def compare_at_size(size_mb):
    """Сравнивает обе реализации на данном размере."""
    size_bytes = size_mb * 1024 * 1024
    data = os.urandom(size_bytes)

    # Тестируем обе реализации
    rayon_time, rayon_len = benchmark_implementation('encode', data)
    pipeline_time, pipeline_len = benchmark_implementation('encode_pipeline_py', data)

    # Проверяем корректность
    if rayon_len != pipeline_len:
        return None, None, None, "ERROR: Different output lengths!"

    rayon_speed = size_mb / rayon_time
    pipeline_speed = size_mb / pipeline_time
    speedup = (rayon_time / pipeline_time - 1) * 100  # % разница

    return rayon_speed, pipeline_speed, speedup, None

def main():
    print("🔬 Rayon vs Pipeline Implementation Benchmark")
    print("=" * 80)
    print(f"{'Size':<10} {'Rayon (MB/s)':<15} {'Pipeline (MB/s)':<18} {'Difference':<15}")
    print("-" * 80)

    # Тестируем размеры от 5MB до 100MB
    sizes = [5, 10, 15, 20, 25, 30, 40, 50, 60, 70, 80, 90, 100]

    results = []
    for size_mb in sizes:
        try:
            rayon_speed, pipeline_speed, speedup, error = compare_at_size(size_mb)

            if error:
                print(f"{size_mb:>3} MB     {error}")
                continue

            # Определяем кто быстрее
            if speedup > 0:
                diff_str = f"Pipeline +{speedup:.1f}%"
            elif speedup < 0:
                diff_str = f"Rayon +{abs(speedup):.1f}%"
            else:
                diff_str = "Equal"

            print(f"{size_mb:>3} MB     {rayon_speed:>8.2f}       {pipeline_speed:>8.2f}          {diff_str}")

            results.append({
                'size': size_mb,
                'rayon': rayon_speed,
                'pipeline': pipeline_speed,
                'speedup': speedup
            })

        except Exception as e:
            print(f"{size_mb:>3} MB     ERROR: {e}")

    # Анализ результатов
    if results:
        print("\n" + "=" * 80)
        print("📊 Analysis:")
        print("-" * 80)

        avg_rayon = sum(r['rayon'] for r in results) / len(results)
        avg_pipeline = sum(r['pipeline'] for r in results) / len(results)

        print(f"Average Rayon speed:    {avg_rayon:.2f} MB/s")
        print(f"Average Pipeline speed: {avg_pipeline:.2f} MB/s")
        print()

        # Находим оптимальный размер для каждого подхода
        rayon_best = max(results, key=lambda r: r['rayon'])
        pipeline_best = max(results, key=lambda r: r['pipeline'])

        print(f"Rayon best:    {rayon_best['rayon']:.2f} MB/s at {rayon_best['size']} MB")
        print(f"Pipeline best: {pipeline_best['pipeline']:.2f} MB/s at {pipeline_best['size']} MB")
        print()

        # Считаем в скольких случаях pipeline выигрывает
        pipeline_wins = sum(1 for r in results if r['speedup'] > 0)
        rayon_wins = sum(1 for r in results if r['speedup'] < 0)

        print(f"Pipeline faster in {pipeline_wins}/{len(results)} cases")
        print(f"Rayon faster in {rayon_wins}/{len(results)} cases")

if __name__ == "__main__":
    main()
