.PHONY: build install test clean dev release

# Команды для разработки
dev:
	maturin develop

dev-release:
	maturin develop --release

# Сборка колес для разных платформ
build:
	maturin build --release

build-all:
	maturin build --release --target x86_64-unknown-linux-gnu
	maturin build --release --target i686-unknown-linux-gnu
	maturin build --release --target aarch64-unknown-linux-gnu

# Установка
install:
	pip install .

# Тестирование
test: dev
	python test_ultrabase64.py

benchmark: dev-release
	python test_ultrabase64.py

# Публикация
publish:
	maturin publish

publish-test:
	maturin publish -r testpypi

# Очистка
clean:
	cargo clean
	rm -rf dist/
	rm -rf target/
	rm -rf *.egg-info/

# Проверка кода
check:
	cargo check
	cargo clippy

fmt:
	cargo fmt

# Документация
doc:
	cargo doc --open

# Полная проверка перед релизом
pre-release: fmt check test benchmark
	@echo "Ready for release!"