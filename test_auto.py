#!/usr/bin/env python3
"""Тест автоматического выбора алгоритма."""

import os
import time
import ultrabase64

def benchmark_implementation(func_name, data):
    """Бенчмарк конкретной реализации."""
    func = getattr(ultrabase64, func_name)

    # Прогрев
    _ = func(data)

    # Основной тест - 3 итерации
    times = []
    for _ in range(3):
        start = time.perf_counter()
        result = func(data)
        end = time.perf_counter()
        times.append(end - start)

    best_time = min(times)
    return best_time, len(result)

def compare_at_size(size_mb):
    """Сравнивает все три реализации на данном размере."""
    size_bytes = size_mb * 1024 * 1024
    data = os.urandom(size_bytes)

    # Тестируем все реализации
    rayon_time, rayon_len = benchmark_implementation('encode', data)
    pipeline_time, pipeline_len = benchmark_implementation('encode_pipeline_py', data)
    auto_time, auto_len = benchmark_implementation('encode_auto', data)

    # Проверяем корректность
    if not (rayon_len == pipeline_len == auto_len):
        return None, None, None, "ERROR: Different output lengths!"

    rayon_speed = size_mb / rayon_time
    pipeline_speed = size_mb / pipeline_time
    auto_speed = size_mb / auto_time

    return rayon_speed, pipeline_speed, auto_speed, None

def main():
    print("🔬 Auto Algorithm Selection Benchmark")
    print("=" * 95)
    print(f"{'Size':<10} {'Rayon':<15} {'Pipeline':<15} {'Auto':<15} {'Best':<15}")
    print(f"{'':10} {'(MB/s)':<15} {'(MB/s)':<15} {'(MB/s)':<15} {'Choice':<15}")
    print("-" * 95)

    sizes = [1, 2, 5, 10, 15, 20, 25, 30, 40, 50, 60, 70, 80, 90, 100]

    results = []
    for size_mb in sizes:
        try:
            rayon_speed, pipeline_speed, auto_speed, error = compare_at_size(size_mb)

            if error:
                print(f"{size_mb:>3} MB     {error}")
                continue

            # Определяем лучший алгоритм
            best_speed = max(rayon_speed, pipeline_speed)
            if rayon_speed > pipeline_speed:
                best_algo = "Rayon"
            else:
                best_algo = "Pipeline"

            # Проверяем насколько Auto близок к лучшему
            auto_efficiency = (auto_speed / best_speed) * 100

            print(f"{size_mb:>3} MB     "
                  f"{rayon_speed:>8.2f}      "
                  f"{pipeline_speed:>8.2f}       "
                  f"{auto_speed:>8.2f}       "
                  f"{best_algo} ({auto_efficiency:.1f}%)")

            results.append({
                'size': size_mb,
                'rayon': rayon_speed,
                'pipeline': pipeline_speed,
                'auto': auto_speed,
                'best': best_speed,
                'best_algo': best_algo
            })

        except Exception as e:
            print(f"{size_mb:>3} MB     ERROR: {e}")

    # Анализ результатов
    if results:
        print("\n" + "=" * 95)
        print("📊 Analysis:")
        print("-" * 95)

        # Средняя эффективность Auto
        avg_auto_efficiency = sum(r['auto'] / r['best'] * 100 for r in results) / len(results)
        print(f"Average Auto efficiency: {avg_auto_efficiency:.2f}% of best")

        # Считаем сколько раз Auto выбрал правильный алгоритм
        # Auto должен:
        # - Использовать Rayon для < 20MB
        # - Использовать Pipeline для >= 20MB (за пределами cache)
        auto_optimal_count = 0
        for r in results:
            size = r['size']
            # Оцениваем выбор Auto по производительности
            if r['auto'] >= r['best'] * 0.95:  # В пределах 5% от лучшего
                auto_optimal_count += 1

        print(f"Auto within 5% of best: {auto_optimal_count}/{len(results)} cases ({auto_optimal_count/len(results)*100:.1f}%)")

        # Средние скорости
        avg_rayon = sum(r['rayon'] for r in results) / len(results)
        avg_pipeline = sum(r['pipeline'] for r in results) / len(results)
        avg_auto = sum(r['auto'] for r in results) / len(results)

        print()
        print(f"Average Rayon:    {avg_rayon:.2f} MB/s")
        print(f"Average Pipeline: {avg_pipeline:.2f} MB/s")
        print(f"Average Auto:     {avg_auto:.2f} MB/s")

if __name__ == "__main__":
    main()
