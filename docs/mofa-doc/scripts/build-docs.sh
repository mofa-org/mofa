#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

EN_CONFIG="book.toml"
ZH_CONFIG="book.zh.toml"
BACKUP_CONFIG=".book.toml.en.backup"

restore_config() {
  if [[ -f "${BACKUP_CONFIG}" ]]; then
    mv -f "${BACKUP_CONFIG}" "${EN_CONFIG}"
  fi
}

trap restore_config EXIT

rm -rf book

# Build English docs at book/
mdbook build

# mdbook + pandoc may emit HTML into book/html; normalize to book/ for GitHub Pages.
if [[ -d "book/html" ]]; then
  cp -a book/html/. book/
  rm -rf book/html
fi

# Build Chinese docs at book/zh/
cp "${EN_CONFIG}" "${BACKUP_CONFIG}"
cp "${ZH_CONFIG}" "${EN_CONFIG}"
mdbook build
restore_config
trap - EXIT

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

echo "Built docs successfully:"
echo "  - English: ${ROOT_DIR}/book"
echo "  - Chinese: ${ROOT_DIR}/book/zh"
