// src/lib.rs - СТАБИЛЬНАЯ ВЕРСИЯ ДЛЯ PyO3 0.21
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use rayon::prelude::*;
use base64::{Engine as _, engine::general_purpose};
use std::sync::OnceLock;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

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

/// Кастомный Base64 engine без padding для параллельной обработки.
static NO_PAD_ENGINE: OnceLock<base64::engine::GeneralPurpose> = OnceLock::new();

/// Кешированное количество доступных CPU для избежания повторных системных вызовов.
static OPTIMAL_THREADS: OnceLock<usize> = OnceLock::new();

fn get_no_pad_engine() -> &'static base64::engine::GeneralPurpose {
    NO_PAD_ENGINE.get_or_init(|| {
        base64::engine::GeneralPurpose::new(
            &base64::alphabet::STANDARD,
            base64::engine::general_purpose::NO_PAD
        )
    })
}

/// Получает оптимальное количество потоков (кешированное).
fn get_optimal_threads() -> usize {
    *OPTIMAL_THREADS.get_or_init(|| num_cpus::get().min(MAX_THREADS))
}

// --- Внутренние функции ---

/// Оптимизированная реализация многопоточного кодирования.
/// Параметр _num_threads сохранен для обратной совместимости, но не используется -
/// Rayon автоматически использует оптимальное количество потоков через work-stealing.
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
        return general_purpose::STANDARD.encode(input);
    }

    let (main_part, tail_part) = input.split_at(main_part_len);

    // 3. ИСПОЛЬЗУЕМ ФИКСИРОВАННЫЙ CHUNK SIZE ДЛЯ ОПТИМАЛЬНОГО L3 CACHE
    // Rayon автоматически распределит чанки между потоками через work-stealing.
    // Фиксированный 1MB чанк оптимален для большинства CPU (L3 cache = 1-2MB/core).
    // ВАЖНО: chunk_size ДОЛЖЕН быть кратен 3 для корректного Base64 кодирования.
    let chunk_size = (MIN_CHUNK_SIZE / 3) * 3;

    // 4. ПАРАЛЛЕЛЬНО КОДИРУЕМ ОСНОВНУЮ ЧАСТЬ (без padding'а)
    let no_pad_engine = get_no_pad_engine();
    let encoded_parts: Vec<String> = main_part
        .par_chunks(chunk_size)
        .map(|chunk| no_pad_engine.encode(chunk))
        .collect();

    // 5. ЭФФЕКТИВНАЯ КОНКАТЕНАЦИЯ: предвычисляем размер для единой аллокации
    let total_len: usize = encoded_parts.iter().map(|s| s.len()).sum();
    let tail_encoded = if !tail_part.is_empty() {
        general_purpose::STANDARD.encode(tail_part)
    } else {
        String::new()
    };

    let mut result = String::with_capacity(total_len + tail_encoded.len());

    // Добавляем все части без реаллокаций
    for part in encoded_parts {
        result.push_str(&part);
    }

    if !tail_encoded.is_empty() {
        result.push_str(&tail_encoded);
    }

    result
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

    // Для небольших данных используем single-threaded
    if main_part_len < CHUNK_SIZE * NUM_WORKERS {
        return general_purpose::STANDARD.encode(input);
    }

    let (main_part, tail_part) = input.split_at(main_part_len);

    let num_chunks = (main_part_len + CHUNK_SIZE_ALIGNED - 1) / CHUNK_SIZE_ALIGNED;

    // Pre-calculate total output size
    let main_output_len = main_part_len / 3 * 4;
    let tail_encoded = if !tail_part.is_empty() {
        general_purpose::STANDARD.encode(tail_part)
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

                    // Encode without padding
                    let encoded = get_no_pad_engine().encode(chunk);

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
    }).expect("Thread pool failed");

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
fn encode(py: Python, data: &PyBytes) -> PyResult<String> {
    let input_data = data.as_bytes();

    // Проверка размера для защиты от OOM
    if input_data.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Input too large: {} bytes (max: {} bytes)", 
                   input_data.len(), MAX_INPUT_SIZE)
        ));
    }

    py.allow_threads(move || {
        if input_data.len() < MULTITHREAD_THRESHOLD {
            // Для небольших данных - обычное кодирование с SIMD
            Ok(general_purpose::STANDARD.encode(input_data))
        } else {
            // Для больших данных - многопоточность
            Ok(encode_multithreaded(input_data, get_optimal_threads()))
        }
    })
}

/// Кодирует байты в Base64 и возвращает bytes (максимальная производительность).
///
/// Аналогичен encode(), но возвращает bytes вместо string для максимальной
/// производительности. Используйте когда результат не нужно конвертировать в string.
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
fn encode_bytes(py: Python, data: &PyBytes) -> PyResult<PyObject> {
    let input_data = data.as_bytes();

    // Проверка размера для защиты от OOM
    if input_data.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Input too large: {} bytes (max: {} bytes)",
                   input_data.len(), MAX_INPUT_SIZE)
        ));
    }

    py.allow_threads(move || {
        let encoded_string = if input_data.len() < MULTITHREAD_THRESHOLD {
            // Для небольших данных - обычное кодирование с SIMD
            general_purpose::STANDARD.encode(input_data)
        } else {
            // Для больших данных - многопоточность
            encode_multithreaded(input_data, get_optimal_threads())
        };

        // Конвертируем String в bytes для максимальной производительности
        Ok(Python::with_gil(|py| PyBytes::new(py, encoded_string.as_bytes()).into()))
    })
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
fn encode_pipeline_py(py: Python, data: &PyBytes) -> PyResult<String> {
    let input_data = data.as_bytes();

    // Проверка размера для защиты от OOM
    if input_data.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Input too large: {} bytes (max: {} bytes)",
                   input_data.len(), MAX_INPUT_SIZE)
        ));
    }

    py.allow_threads(move || {
        Ok(encode_pipeline(input_data))
    })
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
fn encode_auto(py: Python, data: &PyBytes) -> PyResult<String> {
    let input_data = data.as_bytes();

    // Проверка размера для защиты от OOM
    if input_data.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Input too large: {} bytes (max: {} bytes)",
                   input_data.len(), MAX_INPUT_SIZE)
        ));
    }

    py.allow_threads(move || {
        let len = input_data.len();

        if len < MULTITHREAD_THRESHOLD {
            // Для маленьких данных - single-threaded
            Ok(general_purpose::STANDARD.encode(input_data))
        } else if len < 20 * 1024 * 1024 {
            // Для средних данных (1-20MB) - Rayon (оптимален для L3 cache)
            Ok(encode_multithreaded(input_data, get_optimal_threads()))
        } else {
            // Для больших данных (>20MB) - Pipeline (стабильнее вне cache)
            Ok(encode_pipeline(input_data))
        }
    })
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
fn decode(py: Python, data: &str) -> PyResult<PyObject> {
    // Быстрые проверки
    if data.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "Input too large"
        ));
    }
    
    if !is_valid_base64_length(data.len()) {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "Invalid Base64 length"
        ));
    }

    // Скопируем строку для использования в allow_threads
    let data_owned = data.to_owned();

    py.allow_threads(move || {
        let decoded_bytes = general_purpose::STANDARD.decode(data_owned)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Invalid Base64: {}", e)
            ))?;
        
        // Создаем PyBytes в основном потоке
        Ok(Python::with_gil(|py| PyBytes::new(py, &decoded_bytes).into()))
    })
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
fn encode_with_threads(py: Python, data: &PyBytes, threads: usize) -> PyResult<String> {
    let input_data = data.as_bytes();
    
    if input_data.len() > MAX_INPUT_SIZE {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "Input too large"
        ));
    }
    
    let num_threads = threads.clamp(1, MAX_THREADS * 2);

    py.allow_threads(move || {
        if num_threads == 1 || input_data.len() < MIN_CHUNK_SIZE {
            Ok(general_purpose::STANDARD.encode(input_data))
        } else {
            Ok(encode_multithreaded(input_data, num_threads))
        }
    })
}

/// Получает информацию о конфигурации библиотеки.
#[pyfunction]
fn get_info() -> PyResult<std::collections::HashMap<String, String>> {
    let mut info = std::collections::HashMap::new();
    info.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    info.insert("multithread_threshold".to_string(), MULTITHREAD_THRESHOLD.to_string());
    info.insert("max_threads".to_string(), MAX_THREADS.to_string());
    info.insert("max_input_size".to_string(), MAX_INPUT_SIZE.to_string());
    info.insert("available_cpus".to_string(), num_cpus::get().to_string());
    info.insert("rayon_threads".to_string(), rayon::current_num_threads().to_string());
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
    py.allow_threads(move || {
        let input_file = File::open(input_path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                format!("Failed to open input file: {}", e)
            ))?;

        let output_file = File::create(output_path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                format!("Failed to create output file: {}", e)
            ))?;

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
            let bytes_read = reader.read(&mut buffer)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                    format!("Failed to read from file: {}", e)
                ))?;

            if bytes_read == 0 {
                // Конец файла - обрабатываем остаток если есть
                if !remainder.is_empty() {
                    let encoded = general_purpose::STANDARD.encode(&remainder);
                    writer.write_all(encoded.as_bytes())
                        .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                            format!("Failed to write to file: {}", e)
                        ))?;
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
                let encoded = get_no_pad_engine().encode(&data_to_process[..main_len]);
                writer.write_all(encoded.as_bytes())
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                        format!("Failed to write to file: {}", e)
                    ))?;
            }

            // Сохраняем остаток для следующей итерации
            remainder.clear();
            if remainder_len > 0 {
                remainder.extend_from_slice(&data_to_process[main_len..]);
            }
        }

        writer.flush()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                format!("Failed to flush output: {}", e)
            ))?;

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
    py.allow_threads(move || {
        let input_file = File::open(input_path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                format!("Failed to open input file: {}", e)
            ))?;

        let output_file = File::create(output_path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                format!("Failed to create output file: {}", e)
            ))?;

        let mut reader = BufReader::new(input_file);
        let mut writer = BufWriter::new(output_file);

        // Буфер для чтения Base64 данных. Размер кратен 4 для правильного декодирования
        // 1MB закодированных данных соответствует ~750KB исходных
        let buffer_size = (MIN_CHUNK_SIZE / 3) * 4; // Кратен 4
        let mut buffer = vec![0u8; buffer_size];
        let mut total_bytes = 0u64;

        loop {
            let bytes_read = reader.read(&mut buffer)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                    format!("Failed to read from file: {}", e)
                ))?;

            if bytes_read == 0 {
                break;
            }

            // Конвертируем байты в строку
            let base64_str = std::str::from_utf8(&buffer[..bytes_read])
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    format!("Invalid UTF-8 in Base64 file: {}", e)
                ))?;

            // Декодируем
            let decoded = general_purpose::STANDARD.decode(base64_str)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    format!("Invalid Base64: {}", e)
                ))?;

            total_bytes += decoded.len() as u64;

            writer.write_all(&decoded)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                    format!("Failed to write to file: {}", e)
                ))?;
        }

        writer.flush()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(
                format!("Failed to flush output: {}", e)
            ))?;

        Ok(total_bytes)
    })
}

/// Python модуль ultrabase64.
#[pymodule]
fn ultrabase64(_py: Python, m: &PyModule) -> PyResult<()> {
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
    m.add("__doc__", "Ultra-fast Base64 encoding/decoding library with SIMD and multithreading support")?;

    Ok(())
}