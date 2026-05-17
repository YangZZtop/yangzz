#!/usr/bin/env bash
# bumpversion.sh — 一键同步 yangzz 所有版本号文件，并可选地 commit + tag + push
#
# 用法：
#   ./scripts/bumpversion.sh 0.5.0
#   ./scripts/bumpversion.sh 0.5.0 --release   # 自动 commit + tag + push
#
# 会更新的文件：
#   - Cargo.toml                   (workspace.package version)
#   - npm/package.json             (root version + optionalDependencies)
#   - sdk/typescript/package.json
#   - sdk/python/setup.py
#
# 安全：
#   - 校验版本号格式 X.Y.Z (允许 -alpha.1 / -rc.2 等后缀)
#   - 校验 git 工作区干净（除非用 --force）
#   - 跑 cargo build --release 验证
#   - --release 模式才会 push（默认只改文件）

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# ── 解析参数 ─────────────────────────────
NEW_VERSION="${1:-}"
RELEASE_MODE=false
FORCE=false

shift || true
for arg in "$@"; do
  case "$arg" in
    --release) RELEASE_MODE=true ;;
    --force)   FORCE=true ;;
    *) echo "未知参数: $arg"; exit 1 ;;
  esac
done

if [[ -z "$NEW_VERSION" ]]; then
  echo "用法: $0 <new-version> [--release] [--force]"
  echo "示例: $0 0.5.0"
  echo "      $0 0.5.0 --release   # 同时 commit + tag + push"
  exit 1
fi

# 校验版本号格式
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  echo "❌ 版本号格式错误: $NEW_VERSION (应为 X.Y.Z 或 X.Y.Z-suffix)"
  exit 1
fi

# 校验 git 工作区干净
if ! $FORCE && [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
  echo "❌ git 工作区不干净，请先 commit 或 stash。或加 --force 跳过此检查。"
  git status --short
  exit 1
fi

# ── 读旧版本 ─────────────────────────────
OLD_VERSION=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/version = "(.+)"/\1/')

if [[ "$OLD_VERSION" == "$NEW_VERSION" ]]; then
  echo "❌ 新版本号 $NEW_VERSION 与当前一致，没必要 bump。"
  exit 1
fi

echo "🔄 升级版本: $OLD_VERSION → $NEW_VERSION"
echo

# ── 修改文件 ─────────────────────────────
update_file() {
  local file="$1"
  local pattern="$2"
  local replacement="$3"

  if [[ ! -f "$file" ]]; then
    echo "  ⚠️  跳过（文件不存在）: $file"
    return
  fi

  # macOS sed 和 GNU sed 都兼容的写法
  if sed -i.bak -E "$pattern" "$file"; then
    rm -f "${file}.bak"
    echo "  ✅ $file"
  else
    echo "  ❌ $file (sed 失败)"
    exit 1
  fi
}

update_file "Cargo.toml" \
  "0,/^version = \"$OLD_VERSION\"/s//version = \"$NEW_VERSION\"/"

update_file "npm/package.json" \
  "s/\"version\": \"$OLD_VERSION\"/\"version\": \"$NEW_VERSION\"/"

update_file "sdk/typescript/package.json" \
  "s/\"version\": \"$OLD_VERSION\"/\"version\": \"$NEW_VERSION\"/"

update_file "sdk/python/setup.py" \
  "s/version=\"$OLD_VERSION\"/version=\"$NEW_VERSION\"/"

echo

# ── 验证 ─────────────────────────────────
echo "🔍 校验所有版本号已同步..."
EXPECTED_COUNT=4
ACTUAL_COUNT=$(grep -E "^version = \"$NEW_VERSION\"|\"version\": \"$NEW_VERSION\"|version=\"$NEW_VERSION\"" \
  Cargo.toml npm/package.json sdk/typescript/package.json sdk/python/setup.py | wc -l | tr -d ' ')

if [[ "$ACTUAL_COUNT" -ne "$EXPECTED_COUNT" ]]; then
  echo "❌ 版本号同步检查失败：期望 $EXPECTED_COUNT 处匹配，实际 $ACTUAL_COUNT 处"
  echo "   请手动检查上面的文件"
  exit 1
fi
echo "  ✅ 4 个文件版本号一致"
echo

# ── 编译验证 ─────────────────────────────
echo "🔨 cargo build --release 验证..."
if cargo build --release --quiet 2>&1 | tail -5; then
  echo "  ✅ 编译通过"
else
  echo "  ❌ 编译失败，已中止。请检查代码后再次运行。"
  exit 1
fi
echo

# ── Release 模式 ─────────────────────────
if $RELEASE_MODE; then
  echo "📤 Release 模式：commit + tag + push"
  echo

  git add Cargo.toml Cargo.lock npm/package.json sdk/typescript/package.json sdk/python/setup.py
  git commit -m "release: v$NEW_VERSION"
  echo "  ✅ commit 已创建"

  git tag "v$NEW_VERSION"
  echo "  ✅ tag v$NEW_VERSION 已打"

  echo
  echo "📡 推送到 origin（需要代理）..."
  git push origin main
  git push origin "v$NEW_VERSION"
  echo
  echo "🎉 v$NEW_VERSION 已推送！GitHub Actions 正在构建发布。"
  echo "   去这里看进度：https://github.com/YangZZtop/yangzz/actions"
else
  echo "✅ 版本号已升级到 $NEW_VERSION（仅修改文件）"
  echo
  echo "下一步："
  echo "  git diff                       # 查看改动"
  echo "  git add -A && git commit -m \"release: v$NEW_VERSION\""
  echo "  git tag v$NEW_VERSION && git push origin main && git push origin v$NEW_VERSION"
  echo
  echo "或者直接：$0 $NEW_VERSION --release"
fi
