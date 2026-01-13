# Release Guide for ultrabase64 v1.1.0

## ✅ Подготовка завершена

Все изменения закоммичены и запушены в ветку `claude/debug-multithreading-performance-WwruQ`.

---

## 🚀 Способ 1: Автоматический релиз (Рекомендуется)

Используйте подготовленный скрипт:

```bash
./release.sh
```

Скрипт автоматически:
1. ✅ Проверит версию (v1.1.0)
2. ✅ Соберет release build с maturin
3. ✅ Установит локально для тестирования
4. ✅ Запустит smoke test
5. ✅ Создаст git tag v1.1.0 (с подтверждением)
6. ✅ Запушит tag на remote (с подтверждением)

После выполнения:
- Wheel файл: `target/wheels/ultrabase64-1.1.0-*.whl`
- Git tag: `v1.1.0` (создан и запушен)

---

## 🚀 Способ 2: Ручной релиз

### Шаг 1: Собрать релиз

```bash
# Сборка
maturin build --release

# Установка для тестирования
pip install --force-reinstall target/wheels/ultrabase64-1.1.0-*.whl

# Проверка
python3 -c "
import ultrabase64
print(f'Version: {ultrabase64.__version__}')
data = b'Test'
assert ultrabase64.decode(ultrabase64.encode_auto(data)) == data
print('✅ Works!')
"
```

### Шаг 2: Создать git tag

```bash
# Создать аннотированный tag
git tag -a v1.1.0 -m "Release v1.1.0

Major improvements:
- Pipeline architecture with auto-selection
- encode_auto() with 814 MB/s average performance
- 0.17% variance (exceptional stability)
- Full API compatibility with stdlib and fastbase64

See CHANGELOG.md for full details."

# Запушить tag
git push origin v1.1.0
```

### Шаг 3: Создать GitHub Release

1. Перейти на: https://github.com/ruslano69/ultrabase64/releases/new
2. Выбрать tag: `v1.1.0`
3. Release title: `v1.1.0 - Pipeline Architecture & Auto-Selection`
4. Описание (скопировать из CHANGELOG.md или использовать краткую версию ниже)
5. Прикрепить файл: `target/wheels/ultrabase64-1.1.0-*.whl`
6. Нажать "Publish release"

---

## 📝 Краткое описание для GitHub Release

```markdown
## 🚀 Major Features

### New: Auto Algorithm Selection ⭐
- `encode_auto()` - automatically selects optimal algorithm
- **814 MB/s** average performance
- **0.17% variance** (exceptional stability)
- RECOMMENDED for all use cases

### New: Pipeline Architecture
- `encode_pipeline_py()` - explicit pipeline access
- +11% faster on large files (>90MB)
- Zero-copy through crossbeam scoped threads
- Optimal for RAM-bound operations

## 📊 Performance

- **Auto**: 814 MB/s avg (best overall)
- vs stdlib: **2-5x faster**
- vs fastbase64: **+7% avg, +11% on large files**
- **Stability**: 0.17% variance (best in class)

## ✅ API Compatibility

Fully compatible with stdlib and fastbase64:
- `encode()` / `encode_auto()` → returns `str`
- `encode_bytes()` → returns `bytes` (drop-in for fastbase64)
- All outputs byte-identical (verified)

## 📚 Documentation

- [CHANGELOG.md](CHANGELOG.md) - Complete release notes
- [API_COMPATIBILITY.md](API_COMPATIBILITY.md) - Migration guide
- [PIPELINE_ANALYSIS.md](PIPELINE_ANALYSIS.md) - Architecture details
- [STABILITY_REPORT.md](STABILITY_REPORT.md) - Performance analysis

## 🔧 Installation

```bash
pip install ultrabase64-1.1.0-*.whl
```

Or wait for PyPI release:
```bash
pip install --upgrade ultrabase64
```

## 🎯 Quick Start

```python
import ultrabase64

# RECOMMENDED: Auto algorithm selection
encoded = ultrabase64.encode_auto(data)
decoded = ultrabase64.decode(encoded)

# For compatibility with fastbase64
encoded_bytes = ultrabase64.encode_bytes(data)
```

## 🙏 Credits

Thanks to the community for testing and feedback!
```

---

## 📦 (Опционально) Публикация в PyPI

Если вы хотите опубликовать в PyPI:

```bash
# Убедитесь что у вас есть токен PyPI
# Сохраните в ~/.pypirc или используйте переменную окружения

# Публикация
maturin publish
```

Или используйте `twine` вручную:

```bash
pip install twine
twine upload target/wheels/ultrabase64-1.1.0-*.whl
```

---

## ✅ Checklist для релиза

### Pre-release
- [x] Все изменения закоммичены
- [x] Версия обновлена в Cargo.toml (1.1.0)
- [x] CHANGELOG.md создан
- [x] Все тесты проходят
- [x] Бенчмарки подтверждены (3 запуска)
- [x] Документация полная

### Release
- [ ] Собрать релизную версию (`maturin build --release`)
- [ ] Локальное тестирование (`pip install --force-reinstall ...`)
- [ ] Smoke test прошел
- [ ] Git tag создан (`v1.1.0`)
- [ ] Tag запушен на remote
- [ ] GitHub Release создан
- [ ] Wheel прикреплен к релизу

### Post-release (опционально)
- [ ] Опубликовано в PyPI
- [ ] README обновлен с новыми функциями
- [ ] Объявление в social media / форумах

---

## 🎉 После релиза

### Мерж в main

После успешного релиза рекомендуется:

```bash
# Переключиться на main
git checkout main

# Смержить ветку с релизом
git merge claude/debug-multithreading-performance-WwruQ

# Запушить
git push origin main
```

### Обновить документацию

Рассмотрите обновление:
- README.md (добавить примеры с `encode_auto()`)
- Badges (если есть CI/CD)
- Documentation website (если есть)

---

## 📊 Что нового в v1.1.0

### Для пользователей
- ✅ **Используйте `encode_auto()`** - лучшая производительность автоматически
- ✅ Совместимость с stdlib и fastbase64 подтверждена
- ✅ Значительно быстрее (814 MB/s в среднем)
- ✅ Исключительная стабильность (0.17% variance)

### Для разработчиков
- ✅ Pipeline архитектура с crossbeam
- ✅ Полная документация с бенчмарками
- ✅ Automated release script
- ✅ Comprehensive compatibility testing

---

## 🔗 Ссылки

- Repository: https://github.com/ruslano69/ultrabase64
- Releases: https://github.com/ruslano69/ultrabase64/releases
- PyPI: https://pypi.org/project/ultrabase64/ (после публикации)

---

## 💡 Рекомендации

1. **Перед релизом**: Запустите `./release.sh` для автоматизации
2. **После релиза**: Смержите в main и обновите README
3. **Публикация PyPI**: Опционально, но рекомендуется для широкой доступности
4. **Объявление**: Рассмотрите анонс в Python сообществе

Удачного релиза! 🚀
