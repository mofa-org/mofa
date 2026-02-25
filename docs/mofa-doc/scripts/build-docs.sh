#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

ZH_CONFIG="book.zh.toml"
TMP_ZH_BUILD_DIR=""

cleanup() {
  if [[ -n "${TMP_ZH_BUILD_DIR}" && -d "${TMP_ZH_BUILD_DIR}" ]]; then
    rm -rf "${TMP_ZH_BUILD_DIR}"
  fi
}

trap cleanup EXIT

rm -rf book

# Build English docs at book/
mdbook build

# mdbook + pandoc may emit HTML into book/html; normalize to book/ for GitHub Pages.
if [[ -d "book/html" ]]; then
  cp -a book/html/. book/
  rm -rf book/html
fi

# Build Chinese docs at book/zh/
# First, remove the empty zh directory created by English build (from src/zh).
rm -rf book/zh

# Build Chinese docs in an isolated temp directory to avoid mutating book.toml.
TMP_ZH_BUILD_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mofa-doc-zh-build.XXXXXX")"
cp -a src "${TMP_ZH_BUILD_DIR}/src"
if [[ -d theme ]]; then
  cp -a theme "${TMP_ZH_BUILD_DIR}/theme"
fi
cp "${ZH_CONFIG}" "${TMP_ZH_BUILD_DIR}/book.toml"

(
  cd "${TMP_ZH_BUILD_DIR}"
  mdbook build
)

mkdir -p book/zh
cp -a "${TMP_ZH_BUILD_DIR}/book/zh/." book/zh/

# Convenience redirect for /zh.html -> /zh/
cat > book/zh.html <<'HTML'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta http-equiv="refresh" content="0; url=./zh/" />
    <link rel="canonical" href="./zh/" />
    <title>Redirecting...</title>
  </head>
  <body>
    <p>Redirecting to <a href="./zh/">Chinese docs</a>...</p>
  </body>
</html>
HTML

if [[ ! -f "book/zh/introduction.html" ]]; then
  echo "Error: expected Chinese intro page at ${ROOT_DIR}/book/zh/introduction.html" >&2
  exit 1
fi

echo "Built docs successfully:"
echo "  - English: ${ROOT_DIR}/book"
echo "  - Chinese: ${ROOT_DIR}/book/zh"
