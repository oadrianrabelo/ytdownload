#!/usr/bin/env bash
# Linux / macOS: prepara o ambiente (só na primeira vez) e baixa os links de links.txt.
# Uso: ./baixar.sh            (MP4 + MP3)
#      ./baixar.sh -f mp3     (qualquer opção do baixar.py)
set -euo pipefail
cd "$(dirname "$0")"

PY=$(command -v python3 || command -v python || true)
if [ -z "$PY" ]; then
  echo "Python 3 não encontrado. Instale em https://www.python.org/downloads/" >&2
  exit 1
fi

if [ ! -x .venv/bin/python ]; then
  echo "Preparando o ambiente (só na primeira vez)..."
  "$PY" -m venv .venv
fi
echo "Verificando atualizações do yt-dlp..."
.venv/bin/python -m pip install --disable-pip-version-check -q -U -r requirements.txt

.venv/bin/python baixar.py "$@"
