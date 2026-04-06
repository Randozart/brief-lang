@echo off
REM Rebuild Compiler Script
REM Rebuilds the Brief compiler and reinstalls the VSIX extension
REM
REM Usage: rebuild-compiler.bat

setlocal enabledelayedexpansion

set "SCRIPT_DIR=%~dp0"
cd /d "%SCRIPT_DIR%"

echo === Building Brief Compiler ===
cargo build --release
if errorlevel 1 (
    echo Build failed!
    exit /b 1
)

echo.
echo === Building VSIX Extension ===
cd syntax-highlighter

REM Find Node 20 via fnm
set "PATH=%USERPROFILE%\.local\share\fnm;%PATH%"
call fnm use 20

del /q *.vsix 2>nul
call npx vsce package --allow-missing-repository
if errorlevel 1 (
    echo VSIX build failed!
    exit /b 1
)

echo.
echo === Installing Extension ===
copy /y brief-language-0.1.0.vsix ..\brief-language.vsix >nul
call flatpak run com.vscodium.codium --install-extension ..\brief-language.vsix

cd ..
del /q syntax-highlighter\brief-language-0.1.0.vsix 2>nul

echo.
echo === Done ===
echo Reload VSCode/VSCodium: Ctrl+Shift+P -^> Developer: Reload Window

endlocal
