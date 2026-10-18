@echo off
REM Windows: de dois cliques neste arquivo. Ele prepara o ambiente (so na primeira vez)
REM e baixa os links de links.txt em MP4 e MP3.
REM Pelo terminal tambem aceita opcoes: baixar.bat -f mp3
setlocal
cd /d "%~dp0"
chcp 65001 >nul
set PYTHONUTF8=1

if exist ".venv\Scripts\python.exe" goto atualizar
set "PY="
where py >nul 2>nul && set "PY=py -3"
if not defined PY where python >nul 2>nul && set "PY=python"
if not defined PY (
  echo Python nao encontrado.
  echo Instale pelo site https://www.python.org/downloads/ marcando "Add python.exe to PATH",
  echo ou rode no terminal: winget install Python.Python.3.13
  pause
  exit /b 1
)
echo Preparando o ambiente (so na primeira vez)...
%PY% -m venv .venv || (pause & exit /b 1)

:atualizar
echo Verificando atualizacoes do yt-dlp...
".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -q -U -r requirements.txt || (pause & exit /b 1)

".venv\Scripts\python.exe" baixar.py %*
echo.
echo Pronto. Os arquivos estao na pasta "downloads".
pause
