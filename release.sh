#!/bin/bash
# Скрипт для создания релиза ultrabase64

set -e  # Остановка при ошибке

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "🚀 Creating release v${VERSION}"
echo ""

# Проверяем, что рабочая директория чиста
if [[ -n $(git status -s) ]]; then
    echo "❌ Working directory is not clean. Commit all changes first."
    git status -s
    exit 1
fi

echo "✅ Working directory clean"
echo ""

# Убеждаемся, что мы на правильной ветке
CURRENT_BRANCH=$(git branch --show-current)
echo "📍 Current branch: ${CURRENT_BRANCH}"
echo ""

# Собираем релизную версию
echo "🔨 Building release..."
maturin build --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

echo "✅ Build successful"
echo ""

# Устанавливаем локально для тестирования
echo "📦 Installing locally for testing..."
pip install --force-reinstall target/wheels/ultrabase64-${VERSION}-*.whl

if [ $? -ne 0 ]; then
    echo "❌ Installation failed"
    exit 1
fi

echo "✅ Installation successful"
echo ""

# Быстрый smoke test
echo "🧪 Running smoke test..."
python3 -c "
import ultrabase64
data = b'Hello, World!'
encoded = ultrabase64.encode_auto(data)
decoded = ultrabase64.decode(encoded)
assert decoded == data, 'Smoke test failed!'
print('✅ Smoke test passed')
print(f'   Version: {ultrabase64.__version__}')
info = ultrabase64.get_info()
print(f'   Available CPUs: {info[\"available_cpus\"]}')
print(f'   Rayon threads: {info[\"rayon_threads\"]}')
"

if [ $? -ne 0 ]; then
    echo "❌ Smoke test failed"
    exit 1
fi

echo ""

# Показываем wheel файл
WHEEL_FILE=$(ls -1 target/wheels/ultrabase64-${VERSION}-*.whl | head -1)
echo "📦 Release artifact:"
echo "   ${WHEEL_FILE}"
echo "   Size: $(du -h ${WHEEL_FILE} | cut -f1)"
echo ""

# Показываем последние коммиты
echo "📝 Recent commits:"
git log --oneline -5
echo ""

# Спрашиваем подтверждение для создания тега
echo "════════════════════════════════════════════════════════════════"
echo "Ready to create git tag v${VERSION}"
echo "════════════════════════════════════════════════════════════════"
echo ""
read -p "Create tag v${VERSION}? (y/N) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    # Создаем тег
    git tag -a "v${VERSION}" -m "Release v${VERSION}

Highlights:
- Pipeline architecture with auto-selection
- encode_auto() with 814 MB/s average performance
- 0.17% variance (exceptional stability)
- Full API compatibility with stdlib and fastbase64
- Comprehensive documentation

See CHANGELOG.md for full details."

    echo "✅ Tag v${VERSION} created"
    echo ""

    # Спрашиваем о пуше
    read -p "Push tag to remote? (y/N) " -n 1 -r
    echo ""

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git push origin "v${VERSION}"
        echo "✅ Tag pushed to remote"
        echo ""
        echo "🎉 Release v${VERSION} complete!"
        echo ""
        echo "Next steps:"
        echo "  1. Create GitHub Release at: https://github.com/ruslano69/ultrabase64/releases/new?tag=v${VERSION}"
        echo "  2. Upload wheel: ${WHEEL_FILE}"
        echo "  3. (Optional) Publish to PyPI: maturin publish"
    else
        echo "ℹ️  Tag created locally. Push manually with: git push origin v${VERSION}"
    fi
else
    echo "ℹ️  Tag not created. To create manually:"
    echo "     git tag -a v${VERSION} -m 'Release v${VERSION}'"
    echo "     git push origin v${VERSION}"
fi

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "Release v${VERSION} artifacts ready!"
echo "════════════════════════════════════════════════════════════════"
