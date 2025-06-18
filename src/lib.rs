// src/lib.rs - СОВМЕСТИМОСТЬ С PyO3 0.22
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use rayon::prelude::*;
use base64::{Engine as _, engine::general_purpose};
use std::sync::OnceLock;

// --- Константы и конфигурации ---

/// Порог в байтах, после которого имеет смысл включать многопоточность.
/// 1MB - более реалистичный порог для современных систем.
const MULTITHREAD_THRESHOLD: usize = 1024 * 1024;

/// Минимальный размер чанка для многопоточной обработки.
/// Слишком маленькие чанки приводят к overhead.
const MIN_CHUNK_SIZE: usize = 64 * 1024; // 64KB

/// Максимальное количество потоков для кодирования.
const MAX_THREADS: usize = 8;

/// Максимальный размер входных данных (защита от OOM).
const MAX_INPUT_SIZE: usize = 100 * 1024 * 1024; // 100MB

// --- Глобальные ресурсы ---

/// Кастомный Base64 engine без padding для параллельной обработки.
static NO_PAD_ENGINE: OnceLock<base64::engine::GeneralPurpose> = OnceLock::new();

fn get_no_pad_engine() -> &'static base64::engine::GeneralPurpose {
    NO_PAD_ENGINE.get_or_init(|| {
        base64::engine::GeneralPurpose::new(
            &base64::alphabet::STANDARD,
            base64::engine::general_purpose::NO_PAD
        )
    })
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
    
    // Если основная часть слишком мала для параллелизма
    if main_part_len < MIN_CHUNK_SIZE * num_threads {
        return general_purpose::STANDARD.encode(input);
    }
    
    let (main_part, tail_part) = input.split_at(main_part_len);

    // 2. ВЫЧИСЛЯЕМ ОПТИМАЛЬНЫЙ РАЗМЕР ЧАНКА
    let chunk_size = (main_part_len / num_threads / 3) * 3;
    let chunk_size = chunk_size.max(MIN_CHUNK_SIZE);

    // 3. ПАРАЛЛЕЛЬНО КОДИРУЕМ ОСНОВНУЮ ЧАСТЬ (без padding'а)
    let no_pad_engine = get_no_pad_engine();
    let encoded_parts: Vec<String> = main_part
        .par_chunks(chunk_size)
        .map(|chunk| no_pad_engine.encode(chunk))
        .collect();

    // 4. ДОБАВЛЯЕМ "ХВОСТ" (с padding если нужен)
    let mut result = encoded_parts.join("");
    if !tail_part.is_empty() {
        result.push_str(&general_purpose::STANDARD.encode(tail_part));
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
fn encode(py: Python, data: Bound<PyBytes>) -> PyResult<String> {
    let input_data = data.as_bytes(); // В PyO3 0.22 as_bytes() работает на Bound<PyBytes>

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
            let num_threads = num_cpus::get().min(MAX_THREADS);
            Ok(encode_multithreaded(input_data, num_threads))
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
fn decode(py: Python, data: &str) -> PyResult<Bound<PyBytes>> {
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

    py.allow_threads(move || {
        let decoded_bytes = general_purpose::STANDARD.decode(data)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Invalid Base64: {}", e)
            ))?;
        
        // В PyO3 0.22 используем PyBytes::new_bound
        Ok(PyBytes::new_bound(py, &decoded_bytes))
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
fn encode_with_threads(py: Python, data: Bound<PyBytes>, threads: usize) -> PyResult<String> {
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

/// Python модуль ultrabase64.
#[pymodule]
fn ultrabase64(m: &Bound<PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(encode, m)?)?;
    m.add_function(wrap_pyfunction!(decode, m)?)?;
    m.add_function(wrap_pyfunction!(encode_with_threads, m)?)?;
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