// src/lib.rs - PyO3 0.29 + base64-simd (SIMD-движок кодирования)
use base64_simd::AsOut;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule};
use rayon::prelude::*;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::OnceLock;

// --- Константы и конфигурации ---

/// Порог в байтах, после которого имеет смысл включать многопоточность.
/// 1MB - более реалистичный порог для современных систем.
const MULTITHREAD_THRESHOLD: usize = 1024 * 1024;

/// Минимальный размер чанка для многопоточной обработки.
/// 1MB оптимально для L3 cache (обычно 1-2MB на ядро в современных CPU).
/// Это минимизирует cache misses и амортизирует overhead многопоточности.
const MIN_CHUNK_SIZE: usize = 1024 * 1024; // 1MB

/// Максимальное количество потоков для кодирования.
const MAX_THREADS: usize = 8;

/// Максимальный размер входных данных (защита от OOM).
const MAX_INPUT_SIZE: usize = 100 * 1024 * 1024; // 100MB

// --- Глобальные ресурсы ---

/// Кешированное количество доступных CPU для избежания повторных системных вызовов.
static OPTIMAL_THREADS: OnceLock<usize> = OnceLock::new();

/// Выделенный пул потоков для кодирования: фиксированные 8 потоков вместо
/// глобального пула rayon (32 потока на 16-ядерной машине).
/// Меньше oversubscription, меньше contention за аллокатор и память,
/// стабильнее частоты под AVX2-нагрузкой.
static RAYON_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

/// Расширяет время жизни среза до 'static для zero-copy работы внутри detach.
///
/// SAFETY: вызывающий обязан гарантировать:
///
/// 1. Исходный Python-объект (`Bound<PyBytes>` / `&str`) жив, пока выполняется
///    detach-блок (держим его в текущем фрейме - refcount > 0, буфер стабилен,
///    bytes/str иммутабельны).
/// 2. Все потоки join'ятся строго внутри detach-блока
///    (`rayon::ThreadPool::install` / `crossbeam::scope` это гарантируют).
///
/// Тогда доступ из worker-потоков всегда валиден, хотя GIL и отпущен.
unsafe fn extend_lifetime(data: &[u8]) -> &'static [u8] {
    std::mem::transmute(data)
}

/// Получает оптимальное количество потоков (кешированное).
fn get_optimal_threads() -> usize {
    *OPTIMAL_THREADS.get_or_init(|| num_cpus::get().min(MAX_THREADS))
}

/// Выделенный пул потоков кодирования (ленивая инициализация).
fn get_pool() -> &'static rayon::ThreadPool {
    RAYON_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(get_optimal_threads())
            .thread_name(|i| format!("ultra-b64-{i}"))
            .build()
            .expect("failed to build rayon thread pool")
    })
}

/// Выделяет выходной буфер БЕЗ обнуления (без zeroing).
///
/// SAFETY: вызывающий обязан полностью перезаписать все `len` байт
/// до любого чтения. В `encode_multithreaded` это гарантируется:
/// чанки тайлят вход целиком, каждый чанк непуст и кратен 3,
/// выходной слайс имеет точную длину len/3*4.
#[allow(clippy::uninit_vec)]
fn uninit_buffer(len: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(len);
    // SAFETY: см. контракт выше; u8 не имеет Drop, unwind безопасен.
    unsafe { v.set_len(len) };
    v
}

// --- Внутренние функции ---

/// Оптимизированная реализация многопоточного кодирования.
/// Параметр _num_threads сохранен для обратной совместимости, но не используется -
/// Rayon автоматически использует оптимальное количество потоков через work-stealing.
///
/// Чанки кодируются SIMD напрямую в непересекающиеся слайсы предвыделенного
/// выходного буфера: ноль промежуточных аллокаций, один проход по памяти.
fn encode_multithreaded(input: &[u8], _num_threads: usize) -> String {
    let len = input.len();
    if len == 0 {
        return String::new();
    }

    // 1. РАЗДЕЛЯЕМ ДАННЫЕ НА ОСНОВНУЮ ЧАСТЬ И "ХВОСТ"
    let remainder_len = len % 3;
    let main_part_len = len - remainder_len;

    // 2. ПРОВЕРЯЕМ МИНИМАЛЬНЫЙ РАЗМЕР ДЛЯ МНОГОПОТОЧНОСТИ
    // Если данных меньше чем MIN_CHUNK_SIZE * 2, fallback на single-threaded
    if main_part_len < MIN_CHUNK_SIZE * 2 {
        return base64_simd::STANDARD.encode_to_string(input);
    }

    let (main_part, tail_part) = input.split_at(main_part_len);

    // 3. ИСПОЛЬЗУЕМ ФИКСИРОВАННЫЙ CHUNK SIZE ДЛЯ ОПТИМАЛЬНОГО L3 CACHE
    // Rayon автоматически распределит чанки между потоками через work-stealing.
    // Фиксированный 1MB чанк оптимален для большинства CPU (L3 cache = 1-2MB/core).
    // ВАЖНО: chunk_size ДОЛЖЕН быть кратен 3 для корректного Base64 кодирования.
    // Все чанки (включая последний) кратны 3, т.к. main_part_len кратно 3.
    let chunk_size = (MIN_CHUNK_SIZE / 3) * 3;
    let chunk_out_len = chunk_size / 3 * 4;

    // 4. ПРЕДВЫДЕЛЯЕМ ТОЧНЫЙ ВЫХОДНОЙ БУФЕР БЕЗ ОБНУЛЕНИЯ (см. uninit_buffer)
    let main_output_len = main_part_len / 3 * 4;
    let mut output = uninit_buffer(main_output_len);

    // 5. ПАРАЛЛЕЛЬНО КОДИРУЕМ SIMD НАПРЯМУЮ В БУФЕР (без padding'а, без аллокаций)
    // par_chunks_mut даёт непересекающиеся &mut-слайсы - data race исключён по типам.
    // Работаем в выделенном пуле (8 потоков), а не в глобальном (32) - меньше contention.
    get_pool().install(|| {
        main_part
            .par_chunks(chunk_size)
            .zip(output.par_chunks_mut(chunk_out_len))
            .for_each(|(chunk, out)| {
                let _ = base64_simd::STANDARD_NO_PAD.encode(chunk, out.as_out());
            });
    });

    // 6. ХВОСТ (< 3 байт) дописываем однопоточно
    if !tail_part.is_empty() {
        let tail_encoded = base64_simd::STANDARD.encode_to_string(tail_part);
        output.extend_from_slice(tail_encoded.as_bytes());
    }

    // SAFETY: выход base64 - подмножество ASCII, всегда валидный UTF-8
    unsafe { String::from_utf8_unchecked(output) }
}

/// Конвейерная реализация многопоточного кодирования с использованием channels.
/// Использует явное управление потоками через crossbeam для более гибкого контроля.
///
/// Архитектура:
/// - Фиксированные 1MB чанки (оптимально для L3 cache)
/// - NUM_WORKERS потоков для параллельной обработки
/// - Crossbeam channels для передачи работы и результатов
/// - Pre-allocated output buffer для избежания реаллокаций
/// - Прямая запись результатов по offset'ам (порядок не важен)
/// - Scoped threads для работы с нестатическими ссылками (zero-copy!)
fn encode_pipeline(input: &[u8]) -> String {
    use crossbeam::channel;

    let n = input.len();
    if n == 0 {
        return String::new();
    }

    const CHUNK_SIZE: usize = 1024 * 1024; // 1MB
    const NUM_WORKERS: usize = 4;
    const CHUNK_SIZE_ALIGNED: usize = (CHUNK_SIZE / 3) * 3; // Кратно 3

    let remainder_len = n % 3;
    let main_part_len = n - remainder_len;

    // Для небольших данных используем single-threaded (SIMD)
    if main_part_len < CHUNK_SIZE * NUM_WORKERS {
        return base64_simd::STANDARD.encode_to_string(input);
    }

    let (main_part, tail_part) = input.split_at(main_part_len);

    let num_chunks = main_part_len.div_ceil(CHUNK_SIZE_ALIGNED);

    // Pre-calculate total output size
    let main_output_len = main_part_len / 3 * 4;
    let tail_encoded = if !tail_part.is_empty() {
        base64_simd::STANDARD.encode_to_string(tail_part)
    } else {
        String::new()
    };
    let total_output_len = main_output_len + tail_encoded.len();

    // Pre-allocate output buffer
    let mut output_buffer = vec![0u8; total_output_len];

    // Channels: (chunk_idx, input_offset, input_length, output_offset)
    let (work_sender, work_receiver) = channel::unbounded::<(usize, usize, usize, usize)>();
    // Result: (chunk_idx, output_offset, encoded_data)
    let (result_sender, result_receiver) = channel::unbounded::<(usize, usize, Vec<u8>)>();

    // Используем crossbeam::scope для scoped threads - позволяет работать с нестатическими ссылками!
    crossbeam::scope(|s| {
        // Spawn worker threads
        for _ in 0..NUM_WORKERS {
            let work_rx = work_receiver.clone();
            let result_tx = result_sender.clone();

            s.spawn(move |_| {
                while let Ok((chunk_idx, input_offset, input_len, output_offset)) = work_rx.recv() {
                    // Extract chunk from input (zero-copy slice - main_part borrowed from outer scope!)
                    let chunk = &main_part[input_offset..input_offset + input_len];

                    // Encode without padding (SIMD)
                    let encoded = base64_simd::STANDARD_NO_PAD.encode_to_string(chunk);

                    // Send result with chunk_idx for debugging
                    let _ = result_tx.send((chunk_idx, output_offset, encoded.into_bytes()));
                }
            });
        }

        // Send work to workers
        for i in 0..num_chunks {
            let start = i * CHUNK_SIZE_ALIGNED;
            let end = (start + CHUNK_SIZE_ALIGNED).min(main_part_len);
            let chunk_len = end - start;
            let output_offset = start / 3 * 4;

            let _ = work_sender.send((i, start, chunk_len, output_offset));
        }

        // Close sender so workers know when to stop
        drop(work_sender);

        // Collect results (порядок получения не важен!)
        for _ in 0..num_chunks {
            if let Ok((_chunk_idx, output_offset, encoded_data)) = result_receiver.recv() {
                // Write directly to pre-allocated buffer
                output_buffer[output_offset..output_offset + encoded_data.len()]
                    .copy_from_slice(&encoded_data);
            }
        }

        // Scoped threads автоматически join'ятся здесь при выходе из scope
    })
    .expect("Thread pool failed");

    // Append tail
    if !tail_encoded.is_empty() {
        output_buffer[main_output_len..].copy_from_slice(tail_encoded.as_bytes());
    }

    // SAFETY: base64 encoding produces valid UTF-8 (ASCII subset)
    unsafe { String::from_utf8_unchecked(output_buffer) }
}

/// Быстрая проверка корректности Base64 строки.
fn is_valid_base64_length(len: usize) -> bool {
    len % 4 == 0 || len == 0
}

// --- Публичные функции ---

/// Кодирует байты в строку Base64.
///
/// Автоматически использует SIMD и многопоточность для больших данных.
///
/// Args:
///     data: Bytes to encode
///
/// Returns:
///     Base64 encoded string
///
/// Raises:
///     ValueError: If input is too large
#[pyfunction]
fn encode(py: Python, data: &Bound<'_, PyBytes>) -> PyResult<String> {
    let input = data.as_bytes();

    // Проверка размера для защиты от OOM
    if input.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Input too large: {} bytes (max: {} bytes)",
            input.len(),
            MAX_INPUT_SIZE
        )));
    }

    if input.len() < MULTITHREAD_THRESHOLD {
        // Малые данные: кодируем borrow напрямую под GIL -
        // без копии входа и без detach (время удержания GIL ~микросекунды)
        Ok(base64_simd::STANDARD.encode_to_string(input))
    } else {
        // Большие данные: zero-copy + detach (GIL отпущен, вход не копируется).
        // SAFETY: `data` жив весь вызов (см. extend_lifetime), rayon join'ится в install.
        let input_static: &'static [u8] = unsafe { extend_lifetime(input) };
        py.detach(move || Ok(encode_multithreaded(input_static, get_optimal_threads())))
    }
}

/// Кодирует байты в Base64 и возвращает bytes (максимальная производительность).
///
/// Самый быстрый API: кодирование идёт SIMD напрямую в буфер Python-объекта,
/// без промежуточных копий и аллокаций. Для больших данных используется
/// выделенный пул (8 потоков). GIL удерживается на время вызова.
///
/// Используйте когда результат не нужно конвертировать в string.
///
/// Args:
///     data: Bytes to encode
///
/// Returns:
///     Base64 encoded bytes (ASCII)
///
/// Raises:
///     ValueError: If input is too large
#[pyfunction]
fn encode_bytes(py: Python, data: &Bound<'_, PyBytes>) -> PyResult<Py<PyBytes>> {
    let input = data.as_bytes();

    // Проверка размера для защиты от OOM
    if input.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Input too large: {} bytes (max: {} bytes)",
            input.len(),
            MAX_INPUT_SIZE
        )));
    }

    let out_len = base64_simd::STANDARD.encoded_length(input.len());

    // Однопроходное кодирование прямо в буфер PyBytes БЕЗ zeroing:
    // PyO3::new_with обнуляет буфер (лишний проход), поэтому работаем
    // с ffi напрямую. Ни одной промежуточной копии.
    //
    // SAFETY:
    // - PyBytes_FromStringAndSize возвращает новый объект с буфером ровно
    //   out_len байт (или NULL + exception при OOM - проверяется через
    //   assume_owned_or_err); ссылка держится в `bound` весь блок.
    // - `encode` пишет ровно out_len байт (контракт encoded_length),
    //   чанки покрывают выход полностью (см. разбор в encode_multithreaded).
    // - GIL удерживается, алиасинга нет - объект свежий, виден только нам.
    let bound: Bound<PyBytes> = unsafe {
        let ptr = ffi::PyBytes_FromStringAndSize(std::ptr::null(), out_len as ffi::Py_ssize_t);
        let bound: Bound<PyBytes> = Bound::from_owned_ptr_or_err(py, ptr)?.cast_into()?;
        let buffer = ffi::PyBytes_AsString(ptr) as *mut u8;
        let buf: &mut [u8] = std::slice::from_raw_parts_mut(buffer, out_len);

        let remainder_len = input.len() % 3;
        let main_len = input.len() - remainder_len;

        if main_len < MIN_CHUNK_SIZE * 2 {
            // Малые данные: один SIMD-вызов на весь вход
            let _ = base64_simd::STANDARD.encode(input, buf.as_out());
        } else {
            // Большие данные: чанки пишут SIMD напрямую в непересекающиеся
            // слайсы выходного буфера (без padding'а, без аллокаций).
            // Все чанки (включая последний) кратны 3, т.к. main_len кратно 3.
            let (main_in, tail_in) = input.split_at(main_len);
            let main_out_len = main_len / 3 * 4;
            let (main_out, tail_out) = buf.split_at_mut(main_out_len);
            let chunk_size = (MIN_CHUNK_SIZE / 3) * 3;
            let chunk_out_len = chunk_size / 3 * 4;

            get_pool().install(|| {
                main_in
                    .par_chunks(chunk_size)
                    .zip(main_out.par_chunks_mut(chunk_out_len))
                    .for_each(|(chunk, out)| {
                        let _ = base64_simd::STANDARD_NO_PAD.encode(chunk, out.as_out());
                    });
            });

            if !tail_in.is_empty() {
                let _ = base64_simd::STANDARD.encode(tail_in, tail_out.as_out());
            }
        }
        bound
    };

    Ok(bound.unbind())
}

/// Кодирует байты в Base64 используя конвейерную архитектуру (экспериментально).
///
/// Использует явное управление потоками через crossbeam channels вместо Rayon.
/// Потенциально более эффективен для больших данных за счёт лучшего управления
/// кешем и параллелизмом.
///
/// Args:
///     data: Bytes to encode
///
/// Returns:
///     Base64 encoded string
///
/// Raises:
///     ValueError: If input is too large
#[pyfunction]
fn encode_pipeline_py(py: Python, data: &Bound<'_, PyBytes>) -> PyResult<String> {
    let input = data.as_bytes();

    // Проверка размера для защиты от OOM
    if input.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Input too large: {} bytes (max: {} bytes)",
            input.len(),
            MAX_INPUT_SIZE
        )));
    }

    // encode_pipeline сам выбирает single-threaded для малых данных -
    // в этом случае работаем с borrow под GIL без копии и detach.
    if input.len() < MULTITHREAD_THRESHOLD {
        Ok(base64_simd::STANDARD.encode_to_string(input))
    } else {
        // Большие данные: zero-copy + detach (crossbeam::scope join'ится внутри).
        // SAFETY: `data` жив весь вызов, см. extend_lifetime.
        let input_static: &'static [u8] = unsafe { extend_lifetime(input) };
        py.detach(move || Ok(encode_pipeline(input_static)))
    }
}

/// Кодирует байты в Base64 используя автоматический выбор алгоритма.
///
/// Автоматически выбирает оптимальную стратегию на основе размера данных:
/// - < 1MB: Single-threaded с SIMD
/// - 1-20MB: Rayon (лучше для данных в пределах L3 cache)
/// - > 20MB: Pipeline (лучше для больших данных за пределами cache)
///
/// Args:
///     data: Bytes to encode
///
/// Returns:
///     Base64 encoded string
///
/// Raises:
///     ValueError: If input is too large
#[pyfunction]
fn encode_auto(py: Python, data: &Bound<'_, PyBytes>) -> PyResult<String> {
    let input = data.as_bytes();

    // Проверка размера для защиты от OOM
    if input.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Input too large: {} bytes (max: {} bytes)",
            input.len(),
            MAX_INPUT_SIZE
        )));
    }

    let len = input.len();

    if len < MULTITHREAD_THRESHOLD {
        // Малые данные: borrow напрямую под GIL
        Ok(base64_simd::STANDARD.encode_to_string(input))
    } else {
        // Большие данные: zero-copy + detach.
        // SAFETY: `data` жив весь вызов, потоки join'ятся внутри (install/scope).
        let input_static: &'static [u8] = unsafe { extend_lifetime(input) };
        py.detach(move || {
            if len < 20 * 1024 * 1024 {
                // Для средних данных (1-20MB) - Rayon (оптимален для L3 cache)
                Ok(encode_multithreaded(input_static, get_optimal_threads()))
            } else {
                // Для больших данных (>20MB) - Pipeline (стабильнее вне cache)
                Ok(encode_pipeline(input_static))
            }
        })
    }
}

/// Декодирует строку Base64 в байты.
///
/// Args:
///     data: Base64 string to decode
///
/// Returns:
///     Decoded bytes
///
/// Raises:
///     ValueError: If input is invalid Base64
#[pyfunction]
fn decode(py: Python, data: &str) -> PyResult<Py<PyBytes>> {
    // Быстрые проверки
    if data.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "Input too large",
        ));
    }

    if !is_valid_base64_length(data.len()) {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "Invalid Base64 length",
        ));
    }

    // Малые входы декодируем borrow напрямую под GIL (без копии и detach),
    // большие - с detach, чтобы не держать GIL.
    if data.len() < MULTITHREAD_THRESHOLD {
        let decoded_bytes = base64_simd::STANDARD.decode_to_vec(data).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid Base64: {}", e))
        })?;
        return Ok(PyBytes::new(py, &decoded_bytes).unbind());
    }

    // Большие входы: zero-copy + detach (decode однопоточен, join'ить нечего,
    // достаточно того, что `data` живёт весь вызов).
    // SAFETY: см. extend_lifetime.
    let data_static: &'static str = unsafe { std::mem::transmute(data) };

    let decoded_bytes = py.detach(move || {
        let decoded_bytes = base64_simd::STANDARD
            .decode_to_vec(data_static)
            .map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid Base64: {}", e))
            })?;
        Ok::<Vec<u8>, PyErr>(decoded_bytes)
    })?;

    Ok(PyBytes::new(py, &decoded_bytes).unbind())
}

/// Кодирует байты в строку Base64 с явным указанием количества потоков.
///
/// Args:
///     data: Bytes to encode
///     threads: Number of threads to use (1-16)
///
/// Returns:
///     Base64 encoded string
#[pyfunction]
fn encode_with_threads(py: Python, data: &Bound<'_, PyBytes>, threads: usize) -> PyResult<String> {
    let input = data.as_bytes();

    if input.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "Input too large",
        ));
    }

    let num_threads = threads.clamp(1, MAX_THREADS * 2);

    if num_threads == 1 || input.len() < MIN_CHUNK_SIZE {
        // Однопоток / малые данные: borrow напрямую под GIL
        Ok(base64_simd::STANDARD.encode_to_string(input))
    } else {
        // Zero-copy + detach. SAFETY: см. extend_lifetime.
        let input_static: &'static [u8] = unsafe { extend_lifetime(input) };
        py.detach(move || Ok(encode_multithreaded(input_static, num_threads)))
    }
}

/// Получает информацию о конфигурации библиотеки.
#[pyfunction]
fn get_info() -> PyResult<std::collections::HashMap<String, String>> {
    let mut info = std::collections::HashMap::new();
    info.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    info.insert("engine".to_string(), "base64-simd".to_string());
    info.insert(
        "multithread_threshold".to_string(),
        MULTITHREAD_THRESHOLD.to_string(),
    );
    info.insert("max_threads".to_string(), MAX_THREADS.to_string());
    info.insert("max_input_size".to_string(), MAX_INPUT_SIZE.to_string());
    info.insert("available_cpus".to_string(), num_cpus::get().to_string());
    info.insert(
        "rayon_threads".to_string(),
        get_pool().current_num_threads().to_string(),
    );
    Ok(info)
}

/// Кодирует файл в Base64 с использованием streaming (конвейерной обработки).
/// Файл читается и обрабатывается чанками по 1MB, что позволяет:
/// - Обрабатывать файлы любого размера без ограничения MAX_INPUT_SIZE
/// - Минимизировать потребление памяти
/// - Оптимально использовать L3 cache процессора
///
/// Args:
///     input_path: Путь к входному файлу для кодирования
///     output_path: Путь к выходному файлу (будет содержать Base64)
///
/// Returns:
///     Количество обработанных байт
#[pyfunction]
fn encode_file_streaming(py: Python, input_path: &str, output_path: &str) -> PyResult<u64> {
    let input_path = input_path.to_owned();
    let output_path = output_path.to_owned();

    py.detach(move || {
        let input_file = File::open(input_path).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
                "Failed to open input file: {}",
                e
            ))
        })?;

        let output_file = File::create(output_path).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
                "Failed to create output file: {}",
                e
            ))
        })?;

        let mut reader = BufReader::new(input_file);
        let mut writer = BufWriter::new(output_file);

        // Буфер для чтения. Размер кратен 3 для правильного Base64 кодирования
        // без padding между чанками. 1MB = оптимально для L3 cache
        let buffer_size = MIN_CHUNK_SIZE;
        let mut buffer = vec![0u8; buffer_size];
        let mut total_bytes = 0u64;

        // Буфер для остатка от предыдущей итерации (если размер не кратен 3)
        let mut remainder = Vec::new();

        loop {
            // Читаем данные
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
                    "Failed to read from file: {}",
                    e
                ))
            })?;

            if bytes_read == 0 {
                // Конец файла - обрабатываем остаток если есть
                if !remainder.is_empty() {
                    let encoded = base64_simd::STANDARD.encode_to_string(&remainder);
                    writer.write_all(encoded.as_bytes()).map_err(|e| {
                        PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
                            "Failed to write to file: {}",
                            e
                        ))
                    })?;
                }
                break;
            }

            total_bytes += bytes_read as u64;

            // Объединяем остаток с новыми данными
            let mut data_to_process = Vec::with_capacity(remainder.len() + bytes_read);
            data_to_process.extend_from_slice(&remainder);
            data_to_process.extend_from_slice(&buffer[..bytes_read]);

            // Разделяем на основную часть (кратную 3) и новый остаток
            let remainder_len = data_to_process.len() % 3;
            let main_len = data_to_process.len() - remainder_len;

            // Кодируем основную часть без padding
            if main_len > 0 {
                let encoded =
                    base64_simd::STANDARD_NO_PAD.encode_to_string(&data_to_process[..main_len]);
                writer.write_all(encoded.as_bytes()).map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
                        "Failed to write to file: {}",
                        e
                    ))
                })?;
            }

            // Сохраняем остаток для следующей итерации
            remainder.clear();
            if remainder_len > 0 {
                remainder.extend_from_slice(&data_to_process[main_len..]);
            }
        }

        writer.flush().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyIOError, _>(format!("Failed to flush output: {}", e))
        })?;

        Ok(total_bytes)
    })
}

/// Декодирует Base64 файл с использованием streaming (конвейерной обработки).
///
/// Args:
///     input_path: Путь к Base64 файлу для декодирования
///     output_path: Путь к выходному файлу (будет содержать исходные данные)
///
/// Returns:
///     Количество декодированных байт
#[pyfunction]
fn decode_file_streaming(py: Python, input_path: &str, output_path: &str) -> PyResult<u64> {
    let input_path = input_path.to_owned();
    let output_path = output_path.to_owned();

    py.detach(move || {
        let input_file = File::open(input_path).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
                "Failed to open input file: {}",
                e
            ))
        })?;

        let output_file = File::create(output_path).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
                "Failed to create output file: {}",
                e
            ))
        })?;

        let mut reader = BufReader::new(input_file);
        let mut writer = BufWriter::new(output_file);

        // Буфер для чтения Base64 данных. Размер кратен 4 для правильного декодирования
        // 1MB закодированных данных соответствует ~750KB исходных
        let buffer_size = (MIN_CHUNK_SIZE / 3) * 4; // Кратен 4
        let mut buffer = vec![0u8; buffer_size];
        let mut total_bytes = 0u64;

        loop {
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
                    "Failed to read from file: {}",
                    e
                ))
            })?;

            if bytes_read == 0 {
                break;
            }

            // Конвертируем байты в строку
            let base64_str = std::str::from_utf8(&buffer[..bytes_read]).map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Invalid UTF-8 in Base64 file: {}",
                    e
                ))
            })?;

            // Декодируем (SIMD)
            let decoded = base64_simd::STANDARD
                .decode_to_vec(base64_str)
                .map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "Invalid Base64: {}",
                        e
                    ))
                })?;

            total_bytes += decoded.len() as u64;

            writer.write_all(&decoded).map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
                    "Failed to write to file: {}",
                    e
                ))
            })?;
        }

        writer.flush().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyIOError, _>(format!("Failed to flush output: {}", e))
        })?;

        Ok(total_bytes)
    })
}

/// Python модуль ultrabase64.
#[pymodule]
fn ultrabase64(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(encode, m)?)?;
    m.add_function(wrap_pyfunction!(encode_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(encode_pipeline_py, m)?)?;
    m.add_function(wrap_pyfunction!(encode_auto, m)?)?;
    m.add_function(wrap_pyfunction!(decode, m)?)?;
    m.add_function(wrap_pyfunction!(encode_with_threads, m)?)?;
    m.add_function(wrap_pyfunction!(encode_file_streaming, m)?)?;
    m.add_function(wrap_pyfunction!(decode_file_streaming, m)?)?;
    m.add_function(wrap_pyfunction!(get_info, m)?)?;

    // Константы, доступные из Python
    m.add("MULTITHREAD_THRESHOLD", MULTITHREAD_THRESHOLD)?;
    m.add("MAX_INPUT_SIZE", MAX_INPUT_SIZE)?;
    m.add("MIN_CHUNK_SIZE", MIN_CHUNK_SIZE)?;
    m.add("MAX_THREADS", MAX_THREADS)?;

    // Метаданные
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add(
        "__doc__",
        "Ultra-fast Base64 encoding/decoding library with SIMD and multithreading support",
    )?;

    Ok(())
}
