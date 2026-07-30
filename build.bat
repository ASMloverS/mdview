@echo off
setlocal

if /i "%~1"=="-h" goto help
if /i "%~1"=="--help" goto help
if "%~1"=="" set "PROFILE=release" & goto build
if /i "%~1"=="-d" set "PROFILE=debug" & goto build
if /i "%~1"=="--debug" set "PROFILE=debug" & goto build
1>&2 echo Unknown option: "%~1"
call :print_help
exit /b 1

:help
call :print_help
exit /b 0

:build
pushd "%~dp0"
if "%PROFILE%"=="release" (
    call "%~dp0.cargo-vc.bat" build --release
) else (
    call "%~dp0.cargo-vc.bat" build
)
if errorlevel 1 (popd & exit /b 1)
popd

if not exist "%~dp0bin\" mkdir "%~dp0bin" || exit /b 1
copy /y "%~dp0target\%PROFILE%\mdview.exe" "%~dp0bin\mdview.exe" >NUL || exit /b 1
if not exist "%~dp0bin\config.toml" call :write_config
if not exist "%~dp0bin\md-styles\" mkdir "%~dp0bin\md-styles" || exit /b 1
echo Built %~dp0bin\mdview.exe [%PROFILE%]
exit /b 0

:print_help
echo Usage: build.bat [option]
echo.
echo   (no option)    Release build (default)
echo   -d, --debug    Debug build
echo   -h, --help     Show this help and exit
echo.
echo Output: bin\mdview.exe + bin\config.toml + bin\md-styles\
goto :eof

:write_config
set "CFG=%~dp0bin\config.toml"
> "%CFG%" echo # mdview configuration
>> "%CFG%" echo # Theme: builtin name (gruvbox-dark, nord, dracula, ...) or a css file in md-styles/
>> "%CFG%" echo theme = "gruvbox-dark"
>> "%CFG%" echo.
>> "%CFG%" echo # Max content width in columns (comment out for terminal width)
>> "%CFG%" echo # max_width = 100
>> "%CFG%" echo.
>> "%CFG%" echo # Content alignment: center (default) or left; toggle in reader with 'a'
>> "%CFG%" echo # align = "center"
>> "%CFG%" echo.
>> "%CFG%" echo # Reading position history: remember cursor line per file (0 disables)
>> "%CFG%" echo # history_size = 200
>> "%CFG%" echo.
>> "%CFG%" echo # Enable mouse capture in the TUI
>> "%CFG%" echo # mouse = true
goto :eof
