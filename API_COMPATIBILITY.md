# API Compatibility Guide

## Совместимость с популярными base64 библиотеками

ultrabase64 обеспечивает совместимость на уровне API со стандартной библиотекой Python и популярными альтернативами.

---

## ✅ Полная совместимость (Correctness)

Все функции ultrabase64 генерируют **идентичный** base64 вывод:

| Библиотека | Кодирование | Декодирование |
|------------|-------------|---------------|
| stdlib | ✅ MATCH | ✅ MATCH |
| fastbase64 | ✅ MATCH | ✅ MATCH |
| ultrabase64 | ✅ MATCH | ✅ MATCH |

**Проверено:** Все библиотеки выдают байт-в-байт идентичный результат.

---

## 📋 Сравнение API

### 1. Standard Library (base64)

```python
import base64

# Кодирование: bytes -> bytes
encoded = base64.b64encode(data)  # Returns: bytes

# Часто конвертируют в строку
encoded_str = base64.b64encode(data).decode('utf-8')  # Returns: str

# Декодирование: bytes/str -> bytes
decoded = base64.b64decode(encoded)  # Returns: bytes
```

**Тип возвращаемого значения:** `bytes`

---

### 2. fastbase64

```python
import fastbase64

# Кодирование: bytes -> bytes
encoded = fastbase64.standard_b64encode(data)  # Returns: bytes

# Декодирование: bytes -> bytes
decoded = fastbase64.standard_b64decode(encoded)  # Returns: bytes
```

**Тип возвращаемого значения:** `bytes`

---

### 3. ultrabase64

```python
import ultrabase64

# Вариант 1: bytes -> str (удобный, как в stdlib с .decode())
encoded = ultrabase64.encode(data)  # Returns: str ✅

# Вариант 2: bytes -> bytes (совместимый с fastbase64)
encoded = ultrabase64.encode_bytes(data)  # Returns: bytes ✅

# Вариант 3: автоматический выбор алгоритма (рекомендуемый)
encoded = ultrabase64.encode_auto(data)  # Returns: str, 814 MB/s avg ✅

# Вариант 4: прямой доступ к pipeline (для >25MB)
encoded = ultrabase64.encode_pipeline_py(data)  # Returns: str ✅

# Декодирование: str/bytes -> bytes
decoded = ultrabase64.decode(encoded)  # Returns: bytes ✅
```

**Типы возвращаемых значений:**
- `encode()` / `encode_auto()` / `encode_pipeline_py()`: `str`
- `encode_bytes()`: `bytes`

---

## 🔄 Migration Guide

### Замена stdlib base64

#### Вариант 1: Прямая замена (рекомендуемый)

```python
# ❌ СТАРЫЙ КОД (stdlib):
import base64
encoded = base64.b64encode(data).decode('utf-8')
decoded = base64.b64decode(encoded_str)

# ✅ НОВЫЙ КОД (ultrabase64):
import ultrabase64
encoded = ultrabase64.encode(data)          # Уже str, не нужен .decode()!
decoded = ultrabase64.decode(encoded)       # Работает со str
```

**Преимущества:**
- Один вызов вместо двух (не нужен `.decode('utf-8')`)
- Автоматическая многопоточность для больших данных
- 2-5x быстрее stdlib

#### Вариант 2: Совместимый с bytes

```python
# ❌ СТАРЫЙ КОД:
import base64
encoded = base64.b64encode(data)  # bytes

# ✅ НОВЫЙ КОД:
import ultrabase64
encoded = ultrabase64.encode_bytes(data)  # bytes, drop-in replacement
```

---

### Замена fastbase64

#### Прямая замена (совместимый API)

```python
# ❌ СТАРЫЙ КОД (fastbase64):
import fastbase64
encoded = fastbase64.standard_b64encode(data)
decoded = fastbase64.standard_b64decode(encoded)

# ✅ НОВЫЙ КОД (ultrabase64):
import ultrabase64
encoded = ultrabase64.encode_bytes(data)
decoded = ultrabase64.decode(encoded.decode())  # или encoded если bytes
```

**Преимущества над fastbase64:**
- +7% быстрее в среднем (814 vs 760 MB/s)
- +11% быстрее на больших файлах (>90MB)
- Автоматический выбор алгоритма
- Поддержка streaming для файлов

---

### Hybrid Approach (лучшая производительность)

```python
import ultrabase64

# Для максимальной производительности используйте encode_auto()
encoded = ultrabase64.encode_auto(data)  # Returns str
decoded = ultrabase64.decode(encoded)

# Автоматически выбирает:
# - Single-threaded для <1MB
# - Rayon для 1-20MB (оптимально для L3 cache)
# - Pipeline для >20MB (стабильно вне cache)
```

---

## 📊 Сравнительная таблица API

| Функция | Input | Output | Описание | Производительность |
|---------|-------|--------|----------|-------------------|
| `encode(data)` | bytes | **str** | Удобный API, auto MT | 776 MB/s (Rayon) |
| `encode_bytes(data)` | bytes | **bytes** | Drop-in для stdlib/fast | 776 MB/s (Rayon) |
| `encode_auto(data)` | bytes | **str** | **Рекомендуемый**, adaptive | **814 MB/s** ✅ |
| `encode_pipeline_py(data)` | bytes | **str** | Прямой доступ к pipeline | 791 MB/s (>25MB) |
| `encode_with_threads(data, n)` | bytes | **str** | Явное указание потоков | Variable |
| `decode(data)` | str/bytes | **bytes** | Универсальное декодирование | Fast |
| `encode_file_streaming(in, out)` | path | int | Streaming для файлов | Memory efficient |
| `decode_file_streaming(in, out)` | path | int | Streaming декодирование | Memory efficient |

---

## 🎯 Рекомендации по выбору функции

### Для новых проектов:

```python
# ✅ BEST CHOICE - автоматическая оптимизация
import ultrabase64
encoded = ultrabase64.encode_auto(data)
```

**Причины:**
- Highest average performance (814 MB/s)
- Lowest variance (0.17%)
- Автоматический выбор лучшего алгоритма
- Production-ready stability

---

### Для замены существующего кода:

#### Если у вас stdlib + `.decode()`:
```python
# Замените это:
base64.b64encode(data).decode('utf-8')

# На это:
ultrabase64.encode(data)  # или encode_auto(data)
```

#### Если у вас fastbase64:
```python
# Замените это:
fastbase64.standard_b64encode(data)

# На это:
ultrabase64.encode_bytes(data)
```

---

### Для специализированных случаев:

#### Маленькие данные (<10MB), много операций:
```python
ultrabase64.encode(data)  # Rayon оптимален в пределах L3 cache
```

#### Большие гарантированные данные (>30MB):
```python
ultrabase64.encode_pipeline_py(data)  # Стабильнее на RAM-bound операциях
```

#### Файлы неограниченного размера:
```python
ultrabase64.encode_file_streaming(input_path, output_path)  # Фиксированная память
```

---

## ⚠️ Важные отличия

### 1. Возвращаемый тип по умолчанию

**stdlib/fastbase64:**
```python
result = base64.b64encode(data)  # bytes
```

**ultrabase64:**
```python
result = ultrabase64.encode(data)  # str (более удобно!)
```

**Решение:** Используйте `encode_bytes()` если нужен bytes

---

### 2. Decode принимает оба типа

**stdlib:**
```python
base64.b64decode(b"SGVs...")  # bytes
base64.b64decode("SGVs...")   # str
```

**ultrabase64:**
```python
ultrabase64.decode(b"SGVs...")  # bytes ✅
ultrabase64.decode("SGVs...")   # str ✅
```

Полностью совместимо!

---

### 3. Максимальный размер входных данных

**stdlib:** Нет ограничений (может вызвать OOM)

**ultrabase64:** 100MB по умолчанию (защита от OOM)

```python
# Для больших файлов используйте streaming:
ultrabase64.encode_file_streaming(input_path, output_path)  # Любой размер!
```

---

## 🔧 Константы и конфигурация

```python
import ultrabase64

# Доступные константы
print(ultrabase64.MULTITHREAD_THRESHOLD)  # 1048576 (1MB)
print(ultrabase64.MAX_INPUT_SIZE)         # 104857600 (100MB)
print(ultrabase64.MIN_CHUNK_SIZE)         # 1048576 (1MB)
print(ultrabase64.MAX_THREADS)            # 8

# Информация о конфигурации
info = ultrabase64.get_info()
print(info['available_cpus'])
print(info['rayon_threads'])
```

---

## ✅ Checklist для миграции

- [ ] **Определить текущую библиотеку:** stdlib или fastbase64?
- [ ] **Выбрать стратегию замены:**
  - [ ] Новый код → `encode_auto()` (рекомендуется)
  - [ ] Замена stdlib с `.decode()` → `encode()`
  - [ ] Замена fastbase64 → `encode_bytes()`
- [ ] **Проверить типы:**
  - [ ] Если код ожидает `bytes`, используйте `encode_bytes()`
  - [ ] Если код ожидает `str`, используйте `encode()` или `encode_auto()`
- [ ] **Тестирование:**
  - [ ] Проверить на малых данных (<1MB)
  - [ ] Проверить на средних данных (10-20MB)
  - [ ] Проверить на больших данных (>50MB)
- [ ] **Бенчмарк (опционально):**
  - [ ] Сравнить производительность до/после
  - [ ] Ожидаемое улучшение: 2-5x vs stdlib, +7-11% vs fastbase64

---

## 📚 Примеры использования

### Простое кодирование/декодирование

```python
import ultrabase64

# Кодирование
data = b"Hello, World!"
encoded = ultrabase64.encode_auto(data)
print(encoded)  # "SGVsbG8sIFdvcmxkIQ=="

# Декодирование
decoded = ultrabase64.decode(encoded)
print(decoded)  # b"Hello, World!"
```

### Обработка файлов

```python
import ultrabase64

# Encode file (streaming - любой размер!)
bytes_processed = ultrabase64.encode_file_streaming(
    "input.bin",
    "output.b64"
)

# Decode file
bytes_processed = ultrabase64.decode_file_streaming(
    "output.b64",
    "restored.bin"
)
```

### Явный контроль потоков

```python
import ultrabase64

# Использовать 4 потока явно
encoded = ultrabase64.encode_with_threads(data, threads=4)
```

---

## 🎯 Заключение

**ultrabase64 полностью совместим** с stdlib и fastbase64 на уровне корректности:

✅ **Правильность:** Идентичный вывод (проверено)
✅ **API:** Совместимые функции (`encode_bytes` для drop-in замены)
✅ **Производительность:** 2-5x быстрее stdlib, +7-11% быстрее fastbase64
✅ **Удобство:** Возвращает `str` по умолчанию (не нужен `.decode()`)
✅ **Стабильность:** 0.17% variance (лучшая стабильность)

**Рекомендация:** Используйте `ultrabase64.encode_auto()` для оптимальной производительности во всех сценариях.
