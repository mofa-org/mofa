#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIR="${ROOT_DIR}/src"

missing=0

while IFS= read -r line; do
  file="${line%%:*}"
  rest="${line#*:}"
  lineno="${rest%%:*}"
  match="${line##*:}"
  link="${match#](}"
  link="${link%)}"

  case "${link}" in
    http://*|https://*|//*|mailto:*|\#*)
      continue
      ;;
  esac

  dir="$(dirname "${file}")"
  target="${dir}/${link}"
  if [[ ! -f "${target}" ]]; then
    echo "MISSING_LINK ${file}:${lineno} -> ${link}"
    missing=$((missing + 1))
  fi
done < <(rg -n -o '\]\(([^)#]+\.md)\)' "${SRC_DIR}" --glob '*.md')

if [[ "${missing}" -gt 0 ]]; then
  echo "Internal link check failed: ${missing} missing links"
  exit 1
fi

echo "Internal link check passed"
