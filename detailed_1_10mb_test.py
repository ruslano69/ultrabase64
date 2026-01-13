#!/usr/bin/env python3
"""Детальное тестирование производительности в диапазоне 1-10 МБ.

Этот диапазон критичен для понимания:
- Как данные переходят из L3 cache (8MB на i7-7700) в RAM
- Оптимальные точки переключения алгоритмов
- Влияние chunk size на производительность
"""

import os
import time
import ultrabase64
import gc

def benchmark_implementation(func_name, data, iterations=5):
    """Бенчмарк конкретной реализации с несколькими итерациями."""
    func = getattr(ultrabase64, func_name)

    # Прогрев (2 раза)
    for _ in range(2):
        _ = func(data)

    # Основной тест - берем лучшее из iterations
    times = []
    for _ in range(iterations):
        # Принудительная сборка мусора перед тестом
        gc.collect()

        start = time.perf_counter()
        result = func(data)
        end = time.perf_counter()
        times.append(end - start)

    # Берем медиану для стабильности
    times.sort()
    median_time = times[len(times) // 2]
    best_time = times[0]
    worst_time = times[-1]

    return best_time, median_time, worst_time, len(result)

def test_at_size(size_mb, iterations=5):
    """Тестирует все реализации на данном размере."""
    size_bytes = int(size_mb * 1024 * 1024)
    data = os.urandom(size_bytes)

    results = {}

    # Тестируем Rayon (encode)
    best, median, worst, _ = benchmark_implementation('encode', data, iterations)
    results['rayon'] = {
        'best': size_mb / best,
        'median': size_mb / median,
        'worst': size_mb / worst,
    }

    # Тестируем Pipeline
    best, median, worst, _ = benchmark_implementation('encode_pipeline_py', data, iterations)
    results['pipeline'] = {
        'best': size_mb / best,
        'median': size_mb / median,
        'worst': size_mb / worst,
    }

    # Тестируем Auto
    best, median, worst, _ = benchmark_implementation('encode_auto', data, iterations)
    results['auto'] = {
        'best': size_mb / best,
        'median': size_mb / median,
        'worst': size_mb / worst,
    }

    return results

def main():
    print("=" * 100)
    print("🔬 Detailed Performance Analysis: 1-10 MB Range")
    print("=" * 100)
    print()

    # Получаем информацию о системе
    info = ultrabase64.get_info()
    print("System Information:")
    print(f"  Available CPUs: {info['available_cpus']}")
    print(f"  Rayon threads: {info['rayon_threads']}")
    print(f"  Multithread threshold: {info['multithread_threshold']:,} bytes ({info['multithread_threshold']/(1024*1024):.1f} MB)")
    print(f"  Min chunk size: {info['min_chunk_size']:,} bytes ({info['min_chunk_size']/(1024*1024):.1f} MB)")
    print()

    # Детальный диапазон: 1-10 MB с шагом 0.5 MB
    sizes = [
        1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0,
        5.5, 6.0, 6.5, 7.0, 7.5, 8.0, 8.5, 9.0, 9.5, 10.0
    ]

    print("Testing configuration:")
    print(f"  Size range: {sizes[0]}-{sizes[-1]} MB")
    print(f"  Step: 0.5 MB")
    print(f"  Total points: {len(sizes)}")
    print(f"  Iterations per point: 5 (median used)")
    print()
    print("=" * 100)
    print()

    # Заголовок таблицы
    print(f"{'Size':<8} {'Rayon':<12} {'Pipeline':<12} {'Auto':<12} {'Best':<12} {'Winner':<15} {'Margin':<10}")
    print(f"{'(MB)':<8} {'(MB/s)':<12} {'(MB/s)':<12} {'(MB/s)':<12} {'Algo':<12} {'':<15} {'':<10}")
    print("-" * 100)

    all_results = []

    for size_mb in sizes:
        try:
            results = test_at_size(size_mb, iterations=5)

            # Используем медиану для стабильности
            rayon_speed = results['rayon']['median']
            pipeline_speed = results['pipeline']['median']
            auto_speed = results['auto']['median']

            # Определяем победителя
            best_speed = max(rayon_speed, pipeline_speed)
            if rayon_speed > pipeline_speed:
                winner = "Rayon"
                margin = ((rayon_speed / pipeline_speed) - 1) * 100
            else:
                winner = "Pipeline"
                margin = ((pipeline_speed / rayon_speed) - 1) * 100

            # Определяем какой алгоритм лучший
            if rayon_speed > pipeline_speed and rayon_speed > auto_speed:
                best_algo = "Rayon"
            elif pipeline_speed > rayon_speed and pipeline_speed > auto_speed:
                best_algo = "Pipeline"
            else:
                best_algo = "Auto"

            # Auto эффективность относительно лучшего
            auto_efficiency = (auto_speed / best_speed) * 100
            auto_marker = "✅" if auto_efficiency >= 95 else "⚠️" if auto_efficiency >= 90 else "❌"

            print(f"{size_mb:>6.1f}   {rayon_speed:>8.2f}     {pipeline_speed:>8.2f}     "
                  f"{auto_speed:>8.2f}     {best_algo:<12} {winner:<10} {auto_marker}  {margin:>5.1f}%")

            all_results.append({
                'size': size_mb,
                'rayon': rayon_speed,
                'pipeline': pipeline_speed,
                'auto': auto_speed,
                'best_speed': best_speed,
                'winner': winner,
                'margin': margin,
                'auto_efficiency': auto_efficiency,
            })

        except Exception as e:
            print(f"{size_mb:>6.1f} MB ERROR: {e}")

    print()
    print("=" * 100)
    print("📊 Statistical Analysis:")
    print("=" * 100)
    print()

    if all_results:
        # Общая статистика
        rayon_speeds = [r['rayon'] for r in all_results]
        pipeline_speeds = [r['pipeline'] for r in all_results]
        auto_speeds = [r['auto'] for r in all_results]

        print("Average Performance:")
        print(f"  Rayon:    {sum(rayon_speeds)/len(rayon_speeds):>8.2f} MB/s")
        print(f"  Pipeline: {sum(pipeline_speeds)/len(pipeline_speeds):>8.2f} MB/s")
        print(f"  Auto:     {sum(auto_speeds)/len(auto_speeds):>8.2f} MB/s")
        print()

        # Пик производительности
        rayon_best = max(all_results, key=lambda r: r['rayon'])
        pipeline_best = max(all_results, key=lambda r: r['pipeline'])
        auto_best = max(all_results, key=lambda r: r['auto'])

        print("Peak Performance:")
        print(f"  Rayon:    {rayon_best['rayon']:>8.2f} MB/s at {rayon_best['size']:>5.1f} MB")
        print(f"  Pipeline: {pipeline_best['pipeline']:>8.2f} MB/s at {pipeline_best['size']:>5.1f} MB")
        print(f"  Auto:     {auto_best['auto']:>8.2f} MB/s at {auto_best['size']:>5.1f} MB")
        print()

        # Минимум производительности
        rayon_worst = min(all_results, key=lambda r: r['rayon'])
        pipeline_worst = min(all_results, key=lambda r: r['pipeline'])
        auto_worst = min(all_results, key=lambda r: r['auto'])

        print("Minimum Performance:")
        print(f"  Rayon:    {rayon_worst['rayon']:>8.2f} MB/s at {rayon_worst['size']:>5.1f} MB")
        print(f"  Pipeline: {pipeline_worst['pipeline']:>8.2f} MB/s at {pipeline_worst['size']:>5.1f} MB")
        print(f"  Auto:     {auto_worst['auto']:>8.2f} MB/s at {auto_worst['size']:>5.1f} MB")
        print()

        # Variance
        def variance_pct(values):
            avg = sum(values) / len(values)
            var = sum((x - avg) ** 2 for x in values) / len(values)
            return (var ** 0.5 / avg) * 100

        print("Variance (Coefficient of Variation):")
        print(f"  Rayon:    {variance_pct(rayon_speeds):>6.2f}%")
        print(f"  Pipeline: {variance_pct(pipeline_speeds):>6.2f}%")
        print(f"  Auto:     {variance_pct(auto_speeds):>6.2f}%")
        print()

        # Победы
        rayon_wins = sum(1 for r in all_results if r['winner'] == 'Rayon')
        pipeline_wins = sum(1 for r in all_results if r['winner'] == 'Pipeline')

        print("Win Statistics:")
        print(f"  Rayon wins:    {rayon_wins:>2}/{len(all_results)} ({rayon_wins/len(all_results)*100:.1f}%)")
        print(f"  Pipeline wins: {pipeline_wins:>2}/{len(all_results)} ({pipeline_wins/len(all_results)*100:.1f}%)")
        print()

        # Auto эффективность
        auto_efficiencies = [r['auto_efficiency'] for r in all_results]
        auto_optimal_count = sum(1 for e in auto_efficiencies if e >= 95)

        print("Auto Algorithm Performance:")
        print(f"  Average efficiency: {sum(auto_efficiencies)/len(auto_efficiencies):>6.2f}%")
        print(f"  Within 5% of best:  {auto_optimal_count}/{len(all_results)} ({auto_optimal_count/len(all_results)*100:.1f}%)")
        print()

        # Критические точки
        print("Critical Transition Points:")
        print("-" * 50)

        # Находим где Rayon перестает доминировать
        rayon_dominance = [i for i, r in enumerate(all_results) if r['winner'] == 'Rayon']
        if rayon_dominance:
            last_rayon = rayon_dominance[-1]
            if last_rayon < len(all_results) - 1:
                transition_size = all_results[last_rayon]['size']
                print(f"  Rayon → Pipeline transition: ~{transition_size:.1f} MB")

        # Находим точку максимальной производительности
        overall_best = max(all_results, key=lambda r: max(r['rayon'], r['pipeline']))
        print(f"  Peak performance zone: ~{overall_best['size']:.1f} MB ({max(overall_best['rayon'], overall_best['pipeline']):.2f} MB/s)")

        # L3 cache correlation (для i7-7700: 8MB)
        print()
        print("L3 Cache Correlation (i7-7700: 8MB):")
        print("-" * 50)

        # Сравниваем производительность до и после 8MB
        before_8mb = [r for r in all_results if r['size'] <= 8.0]
        after_8mb = [r for r in all_results if r['size'] > 8.0]

        if before_8mb and after_8mb:
            avg_before = sum(r['pipeline'] for r in before_8mb) / len(before_8mb)
            avg_after = sum(r['pipeline'] for r in after_8mb) / len(after_8mb)

            print(f"  Avg Pipeline speed ≤8MB:  {avg_before:>8.2f} MB/s")
            print(f"  Avg Pipeline speed >8MB:  {avg_after:>8.2f} MB/s")

            if avg_after > avg_before:
                print(f"  Performance INCREASES: +{((avg_after/avg_before)-1)*100:.1f}% ✅")
                print("  → No cache cliff! Data likely prefetched efficiently")
            else:
                print(f"  Performance DECREASES: {((avg_before/avg_after)-1)*100:.1f}% ⚠️")
                print("  → Cache exhaustion detected")

    print()
    print("=" * 100)
    print("💡 Recommendations:")
    print("=" * 100)
    print()

    if all_results:
        avg_rayon = sum(r['rayon'] for r in all_results) / len(all_results)
        avg_pipeline = sum(r['pipeline'] for r in all_results) / len(all_results)

        if avg_pipeline > avg_rayon * 1.05:
            print("✅ Pipeline is consistently faster (+5% average)")
            print("   Recommendation: Use encode_pipeline_py() for 1-10MB range")
        elif avg_rayon > avg_pipeline * 1.05:
            print("✅ Rayon is consistently faster (+5% average)")
            print("   Recommendation: Use encode() for 1-10MB range")
        else:
            print("⚖️  Performance is similar (within 5%)")
            print("   Recommendation: Use encode_auto() for automatic selection")

        print()
        print("For your specific CPU (i7-7700 with 8MB L3 cache):")

        if pipeline_wins > rayon_wins:
            print(f"  • Pipeline wins {pipeline_wins}/{len(all_results)} times in this range")
            print(f"  • Average advantage: {((avg_pipeline/avg_rayon)-1)*100:.1f}%")
            print(f"  • RECOMMENDED: encode_pipeline_py()")
        else:
            print(f"  • Rayon wins {rayon_wins}/{len(all_results)} times in this range")
            print(f"  • Average advantage: {((avg_rayon/avg_pipeline)-1)*100:.1f}%")
            print(f"  • RECOMMENDED: encode()")

if __name__ == "__main__":
    main()
