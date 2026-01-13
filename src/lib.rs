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
fn encode_multithreaded(input: &[u8], num_threads: usize) -> String {
    let len = input.len();
    if len == 0 {
        return String::new();
    }

    // 1. РАЗДЕЛЯЕМ ДАННЫЕ НА ОСНОВНУЮ ЧАСТЬ И "ХВОСТ"
    let remainder_len = len % 3;
    let main_part_len = len - remainder_len;

    // 2. ДИНАМИЧЕСКИ ОПРЕДЕЛЯЕМ ЭФФЕКТИВНОЕ КОЛИЧЕСТВО ПОТОКОВ
    // Убеждаемся, что каждый поток получит минимум MIN_CHUNK_SIZE
    let effective_threads = (main_part_len / MIN_CHUNK_SIZE).min(num_threads).max(1);

    // Если не хватает данных даже для 2 потоков, используем single-threaded режим
    if effective_threads < 2 {
        return general_purpose::STANDARD.encode(input);
    }

    let (main_part, tail_part) = input.split_at(main_part_len);

    // 3. ВЫЧИСЛЯЕМ РАЗМЕР ЧАНКА (кратный 3 для правильного Base64 кодирования)
    // Распределяем данные равномерно между потоками
    let chunk_size = (main_part_len / effective_threads / 3) * 3;

    // 3. ПАРАЛЛЕЛЬНО КОДИРУЕМ ОСНОВНУЮ ЧАСТЬ (без padding'а)
    let no_pad_engine = get_no_pad_engine();
    let encoded_parts: Vec<String> = main_part
        .par_chunks(chunk_size)
        .map(|chunk| no_pad_engine.encode(chunk))
        .collect();

    // 4. ЭФФЕКТИВНАЯ КОНКАТЕНАЦИЯ: предвычисляем размер для единой аллокации
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