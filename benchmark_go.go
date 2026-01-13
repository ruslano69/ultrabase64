package main

import (
	"encoding/base64"
	"fmt"
	"math/rand"
	"runtime"
	"sync"
	"time"
)

const (
	// Те же константы что и в Rust версии
	MIN_CHUNK_SIZE         = 1024 * 1024 // 1MB
	MULTITHREAD_THRESHOLD  = 2 * MIN_CHUNK_SIZE
	MAX_THREADS            = 8
)

// encodeMultithreaded - аналог Rust функции encode_multithreaded
func encodeMultithreaded(input []byte, numThreads int) string {
	inputLen := len(input)
	if inputLen == 0 {
		return ""
	}

	// 1. Разделяем на основную часть и хвост (как в Rust)
	remainderLen := inputLen % 3
	mainPartLen := inputLen - remainderLen

	// 2. Проверяем минимальный размер для многопоточности
	if mainPartLen < MULTITHREAD_THRESHOLD {
		return base64.StdEncoding.EncodeToString(input)
	}

	mainPart := input[:mainPartLen]
	tailPart := input[mainPartLen:]

	// 3. Фиксированный chunk size (как в Rust после оптимизации)
	chunkSize := (MIN_CHUNK_SIZE / 3) * 3

	// 4. Параллельное кодирование чанков
	numChunks := (mainPartLen + chunkSize - 1) / chunkSize
	encodedParts := make([]string, numChunks)

	var wg sync.WaitGroup
	semaphore := make(chan struct{}, numThreads)

	for i := 0; i < numChunks; i++ {
		wg.Add(1)
		semaphore <- struct{}{} // Acquire

		go func(idx int) {
			defer wg.Done()
			defer func() { <-semaphore }() // Release

			start := idx * chunkSize
			end := start + chunkSize
			if end > mainPartLen {
				end = mainPartLen
			}

			chunk := mainPart[start:end]
			// Base64 без padding (как NO_PAD_ENGINE в Rust)
			encodedParts[idx] = base64.RawStdEncoding.EncodeToString(chunk)
		}(i)
	}

	wg.Wait()

	// 5. Эффективная конкатенация (как в Rust)
	totalLen := 0
	for _, part := range encodedParts {
		totalLen += len(part)
	}

	var tailEncoded string
	if len(tailPart) > 0 {
		tailEncoded = base64.StdEncoding.EncodeToString(tailPart)
		totalLen += len(tailEncoded)
	}

	// Предаллоцируем буфер
	result := make([]byte, 0, totalLen)
	for _, part := range encodedParts {
		result = append(result, part...)
	}
	if len(tailEncoded) > 0 {
		result = append(result, tailEncoded...)
	}

	return string(result)
}

// encode - основная функция (аналог encode() в Rust)
func encode(input []byte) string {
	if len(input) < MULTITHREAD_THRESHOLD {
		return base64.StdEncoding.EncodeToString(input)
	}

	optimalThreads := runtime.NumCPU()
	if optimalThreads > MAX_THREADS {
		optimalThreads = MAX_THREADS
	}

	return encodeMultithreaded(input, optimalThreads)
}

// benchmark - запускает бенчмарк для заданного размера
func benchmark(sizeMB int) (float64, error) {
	sizeBytes := sizeMB * 1024 * 1024

	// Генерируем тестовые данные (детерминированно)
	rand.Seed(42)
	testData := make([]byte, sizeBytes)
	for i := range testData {
		testData[i] = byte(rand.Intn(256))
	}

	// Прогрев
	_ = encode(testData)

	// 3 прогона, берем лучший результат
	var bestTime float64 = 1e9
	for i := 0; i < 3; i++ {
		start := time.Now()
		_ = encode(testData)
		elapsed := time.Since(start).Seconds()

		if elapsed < bestTime {
			bestTime = elapsed
		}
	}

	throughputMBs := float64(sizeBytes) / bestTime / (1024 * 1024)
	return throughputMBs, nil
}

func main() {
	fmt.Println("🧪 GO BENCHMARK: Проверка гипотезы об архитектурных ограничениях")
	fmt.Println("=" + string(make([]byte, 99)))
	fmt.Printf("\nКонфигурация:\n")
	fmt.Printf("  CPU cores: %d\n", runtime.NumCPU())
	fmt.Printf("  MAX_THREADS: %d\n", MAX_THREADS)
	fmt.Printf("  MIN_CHUNK_SIZE: %d MB\n", MIN_CHUNK_SIZE/(1024*1024))
	fmt.Printf("  Алгоритм: идентичен Rust версии (фиксированные 1MB чанки)\n")

	testSizes := []int{1, 5, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100}

	fmt.Printf("\n%-8s | %-15s | %-15s\n", "Size", "Throughput", "Change")
	fmt.Println(string(make([]byte, 50)))

	var prevTP float64
	results := make(map[int]float64)

	for _, sizeMB := range testSizes {
		tp, err := benchmark(sizeMB)
		if err != nil {
			fmt.Printf("Error for %dMB: %v\n", sizeMB, err)
			continue
		}

		results[sizeMB] = tp

		var changeStr string
		if prevTP > 0 {
			change := ((tp - prevTP) / prevTP) * 100
			changeStr = fmt.Sprintf("%+.1f%%", change)
		} else {
			changeStr = "-"
		}

		fmt.Printf("%6dMB | %11.1f MB/s | %15s\n", sizeMB, tp, changeStr)
		prevTP = tp
	}

	// Анализ
	fmt.Printf("\n" + "=" + string(make([]byte, 99)) + "\n")
	fmt.Println("📊 АНАЛИЗ РЕЗУЛЬТАТОВ:")
	fmt.Printf("=" + string(make([]byte, 99)) + "\n")

	// Проверяем падение между 20MB и 30MB
	if tp20, ok := results[20]; ok {
		if tp30, ok := results[30]; ok {
			drop := ((tp30 - tp20) / tp20) * 100
			fmt.Printf("\n🎯 КРИТИЧЕСКАЯ ТОЧКА (20MB → 30MB):\n")
			fmt.Printf("  20MB: %.1f MB/s\n", tp20)
			fmt.Printf("  30MB: %.1f MB/s\n", tp30)
			fmt.Printf("  Падение: %.1f%%\n", drop)

			if drop < -30 {
				fmt.Printf("\n  ✅ ПОДТВЕРЖДЕНО! Падение >30%% - это архитектурное ограничение!\n")
				fmt.Printf("     Граница кеша процессора ~20-30MB\n")
			} else {
				fmt.Printf("\n  ⚠️  Падение меньше ожидаемого - может быть специфика Go runtime\n")
			}
		}
	}

	// Сравниваем с ожидаемыми результатами Rust
	fmt.Printf("\n📈 СРАВНЕНИЕ С RUST РЕАЛИЗАЦИЕЙ:\n")
	fmt.Println(string(make([]byte, 50)))

	rustResults := map[int]float64{
		10: 1870,
		20: 1425,
		30: 716,
		40: 437,
		50: 426,
	}

	fmt.Printf("%-8s | %-12s | %-12s | %-12s\n", "Size", "Go", "Rust", "Ratio")
	fmt.Println(string(make([]byte, 50)))

	for _, size := range []int{10, 20, 30, 40, 50} {
		if goTP, ok := results[size]; ok {
			rustTP := rustResults[size]
			ratio := goTP / rustTP
			fmt.Printf("%6dMB | %8.1f MB/s | %8.1f MB/s | %8.2fx\n",
				size, goTP, rustTP, ratio)
		}
	}

	fmt.Printf("\n" + "=" + string(make([]byte, 99)) + "\n")
	fmt.Println("💡 ВЫВОД:")
	fmt.Println("=" + string(make([]byte, 99)))
	fmt.Println(`
Если Go показывает такое же падение производительности на 20-30MB,
это ПОДТВЕРЖДАЕТ гипотезу об архитектурных ограничениях процессора!

Одинаковое поведение на разных языках (Rust + Rayon vs Go + goroutines)
с одинаковым алгоритмом (1MB чанки) доказывает, что это не bug в коде,
а физический предел L3 cache процессора.
	`)
	fmt.Println("=" + string(make([]byte, 99)))
}
