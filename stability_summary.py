#!/usr/bin/env python3
"""Краткая сводка по стабильности результатов."""

# Данные из 3 запусков (средние значения)
runs = [
    {"name": "Run #1", "rayon": 776.04, "pipeline": 784.86, "auto": 815.19},
    {"name": "Run #2", "rayon": 756.81, "pipeline": 771.54, "auto": 812.06},
    {"name": "Run #3", "rayon": 786.84, "pipeline": 817.36, "auto": 814.88},
]

def calculate_stats(values):
    """Вычисляет среднее и стандартное отклонение."""
    n = len(values)
    mean = sum(values) / n
    variance = sum((x - mean) ** 2 for x in values) / n
    std_dev = variance ** 0.5
    variance_pct = (std_dev / mean) * 100
    return mean, std_dev, variance_pct

print("=" * 80)
print("📊 STABILITY ANALYSIS: 3 Benchmark Runs")
print("=" * 80)
print()

# Собираем данные
rayon_speeds = [r["rayon"] for r in runs]
pipeline_speeds = [r["pipeline"] for r in runs]
auto_speeds = [r["auto"] for r in runs]

# Вычисляем статистику
rayon_mean, rayon_std, rayon_var_pct = calculate_stats(rayon_speeds)
pipeline_mean, pipeline_std, pipeline_var_pct = calculate_stats(pipeline_speeds)
auto_mean, auto_std, auto_var_pct = calculate_stats(auto_speeds)

print("Individual Runs:")
print("-" * 80)
print(f"{'Run':<10} {'Rayon (MB/s)':<15} {'Pipeline (MB/s)':<18} {'Auto (MB/s)':<15}")
print("-" * 80)
for run in runs:
    print(f"{run['name']:<10} {run['rayon']:>8.2f}        {run['pipeline']:>8.2f}           {run['auto']:>8.2f}")

print()
print("=" * 80)
print("Statistical Summary:")
print("-" * 80)
print(f"{'Metric':<20} {'Rayon':<15} {'Pipeline':<18} {'Auto':<15}")
print("-" * 80)
print(f"{'Mean (MB/s)':<20} {rayon_mean:>8.2f}        {pipeline_mean:>8.2f}           {auto_mean:>8.2f}")
print(f"{'Std Dev (MB/s)':<20} {rayon_std:>8.2f}        {pipeline_std:>8.2f}           {auto_std:>8.2f}")
print(f"{'Variance %':<20} {rayon_var_pct:>7.2f}%        {pipeline_var_pct:>7.2f}%          {auto_var_pct:>7.2f}%")
print(f"{'Min (MB/s)':<20} {min(rayon_speeds):>8.2f}        {min(pipeline_speeds):>8.2f}           {min(auto_speeds):>8.2f}")
print(f"{'Max (MB/s)':<20} {max(rayon_speeds):>8.2f}        {max(pipeline_speeds):>8.2f}           {max(auto_speeds):>8.2f}")
print(f"{'Range (MB/s)':<20} {max(rayon_speeds)-min(rayon_speeds):>8.2f}        {max(pipeline_speeds)-min(pipeline_speeds):>8.2f}           {max(auto_speeds)-min(auto_speeds):>8.2f}")

print()
print("=" * 80)
print("🏆 Winner: AUTO ALGORITHM")
print("-" * 80)
print(f"✅ Highest average performance: {auto_mean:.2f} MB/s")
print(f"✅ Lowest variance: {auto_var_pct:.2f}% (vs {rayon_var_pct:.2f}% Rayon, {pipeline_var_pct:.2f}% Pipeline)")
print(f"✅ Most stable: Range {max(auto_speeds)-min(auto_speeds):.2f} MB/s")
print(f"✅ Performance advantage: +{(auto_mean/rayon_mean-1)*100:.1f}% over Rayon")
print(f"✅ Performance advantage: +{(auto_mean/pipeline_mean-1)*100:.1f}% over Pipeline")

print()
print("=" * 80)
print("📊 Relative Performance:")
print("-" * 80)

# Нормализуем относительно Auto
rayon_rel = (rayon_mean / auto_mean) * 100
pipeline_rel = (pipeline_mean / auto_mean) * 100

print(f"Auto:     100.0% {' ' * 40} █████████████████████")
print(f"Pipeline: {pipeline_rel:>5.1f}% {' ' * 40} {'█' * int(pipeline_rel / 5)}")
print(f"Rayon:    {rayon_rel:>5.1f}% {' ' * 40} {'█' * int(rayon_rel / 5)}")

print()
print("=" * 80)
print("💡 Recommendation: Use ultrabase64.encode_auto() as default")
print("=" * 80)
