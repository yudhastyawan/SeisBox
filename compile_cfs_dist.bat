@echo off
setlocal enabledelayedexpansion

echo ===================================================
echo  Building SeisBox CFS for Windows (Release Mode)
echo ===================================================

echo Compiling seisbox_cfs...
cargo build --release --package seisbox_cfs
if %ERRORLEVEL% neq 0 (
    echo Compilation failed!
    exit /b %ERRORLEVEL%
)

set DIST_DIR=dist_win_cfs

echo Creating distribution folder at %DIST_DIR%...
if exist %DIST_DIR% rmdir /S /Q %DIST_DIR%
mkdir %DIST_DIR%

echo Copying seisbox_cfs binary...
if exist target\release\seisbox_cfs.exe (
    copy target\release\seisbox_cfs.exe %DIST_DIR%\ >nul
) else (
    echo Warning: seisbox_cfs.exe not found in target\release\
    exit /b 1
)

echo Copying resources...
for %%F in (docs\tutorial_cfs) do (
    if exist %%F\ (
        echo Including %%F in distribution...
        mkdir %DIST_DIR%\%%F
        xcopy %%F %DIST_DIR%\%%F /E /H /C /I >nul
    )
)

echo Creating zip distribution...
if exist SeisBox_CFS_Windows.zip del SeisBox_CFS_Windows.zip

powershell.exe -nologo -noprofile -command "Compress-Archive -Path '%DIST_DIR%\*' -DestinationPath 'SeisBox_CFS_Windows.zip' -Force"

echo Zip file created at SeisBox_CFS_Windows.zip
echo Done! The application is ready at %DIST_DIR%\
