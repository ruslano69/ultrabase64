#!/usr/bin/env python3
"""Специальный тест для исследования границы L3 cache (8MB на i7-7700).

Этот тест фокусируется на диапазоне 6-10 МБ с очень мелким шагом (0.25MB)
для точного определения момента, когда данные перестают помещаться в L3 cache.
"""

import os
import time
import ultrabase64
import gc

def quick_benchmark(func_name, data):
    """Быстрый бенчмарк с 3 итерациями."""
    func = getattr(ultrabase64, func_name)

    # Прогрев
    _ = func(data)

    # 3 итерации
    times = []
    for _ in range(3):
        gc.collect()
        start = time.perf_counter()
        _ = func(data)
        end = time.perf_counter()
        times.append(end - start)

    return min(times)

def main():
    print("=" * 80)
    print("🔍 L3 Cache Boundary Analysis (i7-7700: 8MB L3 Cache)")
    print("=" * 80)
    print()

    # Очень детальный диапазон вокруг 8MB
    sizes_mb = []

    # 6-7.75 MB (шаг 0.25)
    sizes_mb.extend([6.0 + i * 0.25 for i in range(8)])  # 6.0 - 7.75

    # 7.75-8.25 MB (шаг 0.125) - самая критичная зона!
    sizes_mb.extend([7.75 + i * 0.125 for i in range(1, 5)])  # 7.875 - 8.125

    # 8.25-10 MB (шаг 0.25)
    sizes_mb.extend([8.25 + i * 0.25 for i in range(1, 8)])  # 8.5 - 10.0

    print(f"Testing {len(sizes_mb)} points from {sizes_mb[0]:.3f} MB to {sizes_mb[-1]:.3f} MB")
    print(f"Critical zone: 7.75-8.25 MB (step 0.125 MB)")
    print()
    print("=" * 80)
    print()

    # Заголовок
    print(f"{'Size':<12} {'Rayon':<12} {'Pipeline':<12} {'Auto':<12} {'Winner':<12} {'Status':<15}")
    print(f"{'(MB)':<12} {'(MB/s)':<12} {'(MB/s)':<12} {'(MB/s)':<12} {'':<12} {'':<15}")
    print("-" * 80)

    results = []

    for size_mb in sizes_mb:
        size_bytes = int(size_mb * 1024 * 1024)
        data = os.urandom(size_bytes)

        try:
            # Тестируем все три
            rayon_time = quick_benchmark('encode', data)
            pipeline_time = quick_benchmark('encode_pipeline_py', data)
            auto_time = quick_benchmark('encode_auto', data)

            rayon_speed = size_mb / rayon_time
            pipeline_speed = size_mb / pipeline_time
            auto_speed = size_mb / auto_time

            winner = "Rayon" if rayon_speed > pipeline_speed else "Pipeline"
            margin = abs(rayon_speed - pipeline_speed) / min(rayon_speed, pipeline_speed) * 100

            # Определяем статус относительно L3 cache (8MB)
            if size_mb < 7.75:
                status = "Well in cache"
            elif 7.75 <= size_mb < 8.0:
                status = "Near boundary"
            elif 8.0 <= size_mb < 8.25:
                status = "⚠️  At boundary"
            else:
                status = "Outside cache"

            marker = "🔴" if 7.75 <= size_mb <= 8.25 else ""

            print(f"{size_mb:>10.3f} {marker:1} {rayon_speed:>8.2f}     {pipeline_speed:>8.2f}     "
                  f"{auto_speed:>8.2f}     {winner:<12} {status:<15}")

            results.append({
                'size': size_mb,
                'rayon': rayon_speed,
                'pipeline': pipeline_speed,
                'auto': auto_speed,
                'winner': winner,
                'margin': margin,
            })

        except Exception as e:
            print(f"{size_mb:>10.3f}   ERROR: {e}")

    print()
    print("=" * 80)
    print("📊 Cache Boundary Analysis:")
    print("=" * 80)
    print()

    if results:
        # Разбиваем на зоны
        zone1 = [r for r in results if r['size'] < 7.75]  # До границы
        zone2 = [r for r in results if 7.75 <= r['size'] < 8.25]  # На границе
        zone3 = [r for r in results if r['size'] >= 8.25]  # После границы

        print("Performance by Zone:")
        print("-" * 80)

        if zone1:
            avg1 = sum(r['pipeline'] for r in zone1) / len(zone1)
            print(f"  Zone 1 (< 7.75 MB):      {avg1:>8.2f} MB/s avg (Pipeline)")

        if zone2:
            avg2 = sum(r['pipeline'] for r in zone2) / len(zone2)
            print(f"  Zone 2 (7.75-8.25 MB):   {avg2:>8.2f} MB/s avg (⚠️  Boundary)")

        if zone3:
            avg3 = sum(r['pipeline'] for r in zone3) / len(zone3)
            print(f"  Zone 3 (> 8.25 MB):      {avg3:>8.2f} MB/s avg (Outside)")

        print()

        if zone1 and zone3:
            change = ((avg3 - avg1) / avg1) * 100
            if change > 0:
                print(f"✅ Performance INCREASES across boundary: +{change:.1f}%")
                print("   → No cache cliff detected!")
                print("   → Possible reasons:")
                print("     • Hardware prefetcher is very effective")
                print("     • Windows memory management optimized")
                print("     • Sequential access pattern benefits from prefetch")
            else:
                print(f"⚠️  Performance DECREASES across boundary: {change:.1f}%")
                print("   → Cache exhaustion detected")

        print()

        # Находим точку максимальной скорости
        best = max(results, key=lambda r: r['pipeline'])
        print(f"Peak Pipeline Performance: {best['pipeline']:.2f} MB/s at {best['size']:.3f} MB")

        # Находим точку минимальной скорости
        worst = min(results, key=lambda r: r['pipeline'])
        print(f"Minimum Pipeline Performance: {worst['pipeline']:.2f} MB/s at {worst['size']:.3f} MB")

        print()

        # Variance в каждой зоне
        def calc_variance(values):
            avg = sum(values) / len(values)
            var = sum((x - avg) ** 2 for x in values) / len(values)
            return (var ** 0.5 / avg) * 100

        print("Stability Analysis:")
        print("-" * 80)

        if zone1:
            var1 = calc_variance([r['pipeline'] for r in zone1])
            print(f"  Zone 1 variance: {var1:>6.2f}%")

        if zone2:
            var2 = calc_variance([r['pipeline'] for r in zone2])
            print(f"  Zone 2 variance: {var2:>6.2f}% ⚠️  (boundary)")

        if zone3:
            var3 = calc_variance([r['pipeline'] for r in zone3])
            print(f"  Zone 3 variance: {var3:>6.2f}%")

    print()
    print("=" * 80)
    print("💡 Conclusion:")
    print("=" * 80)
    print()
    print("For i7-7700 with 8MB L3 Cache:")
    print()

    if results:
        # Сравниваем зоны
        if zone1 and zone3:
            if avg3 >= avg1 * 0.95:  # В пределах 5%
                print("✅ NO significant cache cliff detected!")
                print("   Performance remains stable across 8MB boundary")
                print()
                print("   Possible explanations:")
                print("   • Hardware prefetcher compensates effectively")
                print("   • Sequential memory access pattern optimal")
                print("   • Windows memory management well-tuned")
                print("   • Pipeline architecture benefits from fixed worker pool")
            else:
                print("⚠️  Cache cliff detected!")
                print(f"   Performance drops {((avg1-avg3)/avg1)*100:.1f}% after 8MB")

        print()
        print("Recommendation:")

        avg_overall = sum(r['pipeline'] for r in results) / len(results)
        print(f"   Average Pipeline: {avg_overall:.2f} MB/s in 6-10MB range")
        print(f"   Use encode_pipeline_py() for this range on your CPU")

if __name__ == "__main__":
    main()
