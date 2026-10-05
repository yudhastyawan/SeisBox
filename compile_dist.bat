@echo off
setlocal enabledelayedexpansion

echo ===================================================
echo  Building SeisBox Suite for Windows (Release Mode)
echo ===================================================

echo Compiling workspace...
cargo build --release
if %ERRORLEVEL% neq 0 (
    echo Compilation failed!
    exit /b %ERRORLEVEL%
)

set DIST_DIR=dist_win

echo Creating distribution folder at %DIST_DIR%...
if exist %DIST_DIR% rmdir /S /Q %DIST_DIR%
mkdir %DIST_DIR%

echo Copying launcher binary...
if exist target\release\seisbox_launcher.exe (
    copy target\release\seisbox_launcher.exe %DIST_DIR%\SeisBox.exe >nul
) else (
    echo Launcher binary not found!
    exit /b 1
)

echo Copying sub-application binaries...
set APPS=seisbox_picker seisbox_stats seisbox_hvsr seisbox_inversion seisbox_cfs seisbox_interp seisbox_fdsn seisbox_isc

for %%A in (%APPS%) do (
    if exist target\release\%%A.exe (
        copy target\release\%%A.exe %DIST_DIR%\ >nul
    ) else (
        echo Warning: Sub-app %%A.exe not found in target\release\
    )
)

echo Copying resources...
for %%F in (examples docs assets) do (
    if exist %%F\ (
        echo Including %%F in distribution...
        mkdir %DIST_DIR%\%%F
        xcopy %%F %DIST_DIR%\%%F /E /H /C /I >nul
    )
)

echo Creating zip distribution...
if exist SeisBox_Windows.zip del SeisBox_Windows.zip

powershell.exe -nologo -noprofile -command "Compress-Archive -Path '%DIST_DIR%\*' -DestinationPath 'SeisBox_Windows.zip' -Force"

echo Zip file created at SeisBox_Windows.zip
echo Done! The application is ready at %DIST_DIR%\

set /P RUN_NOW="Do you want to run SeisBox now? (y/n) "
if /I "%RUN_NOW%"=="y" (
    start %DIST_DIR%\SeisBox.exe
)
