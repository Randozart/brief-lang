@echo off
REM Brief Compiler Installer for Windows
REM Usage: .\brief-install.bat [--prefix <directory>]

setlocal enabledelayedexpansion

set "INSTALL_PREFIX=%LOCALAPPDATA%\brief"
set "BIN_NAME=brief.exe"
set "BINARY_NAME=brief-compiler.exe"

REM Parse arguments
:parse_args
if "%~1"=="" goto :done_parsing
if "%~1"=="--prefix" (
    set "INSTALL_PREFIX=%~2"
    shift
    shift
    goto :parse_args
)
if "%~1"=="--help" (
    echo Brief Compiler Installer for Windows
    echo.
    echo Usage: %~nx0 [--prefix ^<directory^>]
    echo.
    echo Options:
    echo   --prefix ^<dir^>  Installation directory (default: %LOCALAPPDATA%\brief)
    echo   --help           Show this help message
    echo.
    echo After installation, add the following to your PATH:
    echo   Control Panel -^> System -^> Advanced -^> Environment Variables
    echo.
    echo Then run:
    echo   brief init my-app
    echo   cd my-app
    echo   brief run
    exit /b 0
)
shift
goto :parse_args

:done_parsing

echo Installing Brief compiler...
echo   Target: %INSTALL_PREFIX%\%BIN_NAME%

REM Find the script's directory
set "SCRIPT_DIR=%~dp0"
set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

REM Find the binary
set "BINARY_PATH="
if exist "%SCRIPT_DIR%\target\release\%BINARY_NAME%" (
    set "BINARY_PATH=%SCRIPT_DIR%\target\release\%BINARY_NAME%"
) else if exist "%SCRIPT_DIR%\target\debug\%BINARY_NAME%" (
    set "BINARY_PATH=%SCRIPT_DIR%\target\debug\%BINARY_NAME%"
) else if exist "%SCRIPT_DIR%\%BINARY_NAME%" (
    set "BINARY_PATH=%SCRIPT_DIR%\%BINARY_NAME%"
) else (
    echo.
    echo Error: Could not find Brief compiler binary.
    echo Expected locations:
    echo   - %SCRIPT_DIR%\target\release\%BINARY_NAME%
    echo   - %SCRIPT_DIR%\target\debug\%BINARY_NAME%
    echo.
    echo If you haven't built the compiler yet, download a release or run:
    echo   cargo build --release
    exit /b 1
)

REM Create install directory if needed
if not exist "%INSTALL_PREFIX%" (
    mkdir "%INSTALL_PREFIX%"
)

REM Install the binary
copy /Y "%BINARY_PATH%" "%INSTALL_PREFIX%\%BIN_NAME%" >nul

REM Verify installation
if exist "%INSTALL_PREFIX%\%BIN_NAME%" (
    echo.
    echo Brief installed successfully!
    echo.
    echo Next steps:
    echo   1. Add to your PATH:
    echo        %INSTALL_PREFIX%
    echo      Open: Control Panel -^> System -^> Advanced -^> Environment Variables
    echo.
    echo   2. Create a new project:
    echo        brief init my-app
    echo        cd my-app
    echo.
    echo   3. Run your app:
    echo        brief run
    echo.
    echo   4. Open http://localhost:8080 in your browser
) else (
    echo.
    echo Error: Installation failed
    exit /b 1
)
