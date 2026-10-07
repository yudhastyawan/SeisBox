@echo off
setlocal enabledelayedexpansion

echo ===================================================
echo  SeisBox MSI Installer Builder (Windows)
echo ===================================================
echo.
echo  Requires: WiX Toolset v3 (candle.exe + light.exe)
echo  Download: https://wixtoolset.org/releases/
echo ===================================================
echo.

:: Check if dist_win folder exists (output from compile_dist.bat)
set DIST_DIR=dist_win
if not exist %DIST_DIR% (
    echo Error: %DIST_DIR% folder not found!
    echo Please run compile_dist.bat first to build the binaries.
    exit /b 1
)

:: Check if WiX Toolset is available
where candle.exe >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo WiX Toolset not found in PATH.
    echo Checking common install locations...
    
    set WIX_FOUND=0
    for %%D in (
        "%WIX%bin"
        "%ProgramFiles%\WiX Toolset v7.0\bin"
        "%ProgramFiles(x86)%\WiX Toolset v7.0\bin"
        "%ProgramFiles(x86)%\WiX Toolset v3.14\bin"
        "%ProgramFiles(x86)%\WiX Toolset v3.11\bin"
        "%ProgramFiles%\WiX Toolset v3.14\bin"
        "%ProgramFiles%\WiX Toolset v3.11\bin"
    ) do (
        if exist "%%~D\candle.exe" (
            echo Found WiX at %%~D
            set "PATH=%%~D;!PATH!"
            set WIX_FOUND=1
            goto :wix_found
        )
    )
    
    if !WIX_FOUND! equ 0 (
        echo.
        echo ERROR: WiX Toolset is required but not found.
        echo Please install it from: https://wixtoolset.org/releases/
        echo Or set the WIX environment variable to your install path.
        exit /b 1
    )
)
:wix_found
echo WiX Toolset found.
echo.

:: ---- Configuration ----
set PRODUCT_NAME=SeisBox
set PRODUCT_VERSION=0.1.2
set MANUFACTURER=Yudha Styawan - Geophysical Engineering, Institut Teknologi Sumatera
set UPGRADE_GUID=7B2E8F4A-1C3D-4E5F-A6B7-8C9D0E1F2A3B
set WXS_FILE=seisbox_installer.wxs
set MSI_FILE=SeisBox_Windows_Installer.msi
set OBJ_DIR=wix_obj

:: Create obj directory
if exist %OBJ_DIR% rmdir /S /Q %OBJ_DIR%
mkdir %OBJ_DIR%

:: ---- Generate LICENSE.rtf from LICENSE text file ----
echo Generating LICENSE.rtf...
(
echo {\rtf1\ansi\deff0{\fonttbl{\f0 Consolas;}}
echo \f0\fs18
for /f "usebackq delims=" %%L in ("LICENSE") do (
    echo %%L\par
)
echo }
) > LICENSE.rtf
echo LICENSE.rtf generated.
echo.

echo Generating WiX source file...

:: ---- Build file list from dist_win ----
set FILE_REFS=
set FILE_COMPONENTS=
set COMP_INDEX=0

:: Count executables
set EXE_LIST=
for %%F in (%DIST_DIR%\*.exe) do (
    set /A COMP_INDEX+=1
    set "EXE_LIST=!EXE_LIST! %%~nxF"
)

echo Found executables:!EXE_LIST!
echo.

:: ---- Generate WXS (WiX XML Source) ----
(
echo ^<?xml version="1.0" encoding="UTF-8"?^>
echo ^<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi"^>
echo.
echo   ^<Product Id="*"
echo            Name="%PRODUCT_NAME%"
echo            Language="1033"
echo            Version="%PRODUCT_VERSION%"
echo            Manufacturer="%MANUFACTURER%"
echo            UpgradeCode="%UPGRADE_GUID%"^>
echo.
echo     ^<Package InstallerVersion="200"
echo              Compressed="yes"
echo              InstallScope="perMachine"
echo              Description="%PRODUCT_NAME% %PRODUCT_VERSION% Installer"
echo              Comments="Seismic Analysis Toolbox for Windows" /^>
echo.
echo     ^<MajorUpgrade DowngradeErrorMessage="A newer version of [ProductName] is already installed." /^>
echo     ^<MediaTemplate EmbedCab="yes" /^>
echo.
echo     ^<!-- UI Configuration --^>
echo     ^<UIRef Id="WixUI_InstallDir" /^>
echo     ^<Property Id="WIXUI_INSTALLDIR" Value="INSTALLFOLDER" /^>
echo.
echo     ^<!-- Icon for Add/Remove Programs --^>
echo     ^<Icon Id="SeisBoxIcon.exe" SourceFile="%DIST_DIR%\SeisBox.exe" /^>
echo     ^<Property Id="ARPPRODUCTICON" Value="SeisBoxIcon.exe" /^>
echo.
echo     ^<!-- License Agreement Dialog --^>
echo     ^<WixVariable Id="WixUILicenseRtf" Value="LICENSE.rtf" /^>
echo.
echo     ^<Feature Id="Complete" Title="%PRODUCT_NAME%" Level="1"^>
echo       ^<ComponentGroupRef Id="ProductComponents" /^>
echo       ^<ComponentRef Id="ProgramMenuShortcut" /^>
echo       ^<ComponentRef Id="DesktopShortcut" /^>
echo       ^<ComponentRef Id="AddToPath" /^>
echo       ^<ComponentRef Id="FileAssocMseed" /^>
echo       ^<ComponentRef Id="FileAssocSac" /^>
echo     ^</Feature^>
echo.
echo     ^<Directory Id="TARGETDIR" Name="SourceDir"^>
echo       ^<Directory Id="ProgramFilesFolder"^>
echo         ^<Directory Id="INSTALLFOLDER" Name="%PRODUCT_NAME%"^>
echo         ^</Directory^>
echo       ^</Directory^>
echo       ^<Directory Id="ProgramMenuFolder"^>
echo         ^<Directory Id="ApplicationProgramsFolder" Name="%PRODUCT_NAME%"^>
echo         ^</Directory^>
echo       ^</Directory^>
echo       ^<Directory Id="DesktopFolder" Name="Desktop"^>
echo       ^</Directory^>
echo     ^</Directory^>
echo.
echo     ^<!-- Start Menu Shortcut --^>
echo     ^<DirectoryRef Id="ApplicationProgramsFolder"^>
echo       ^<Component Id="ProgramMenuShortcut" Guid="A1B2C3D4-E5F6-7890-ABCD-EF1234567890"^>
echo         ^<Shortcut Id="StartMenuShortcut"
echo                   Name="%PRODUCT_NAME%"
echo                   Description="Launch %PRODUCT_NAME%"
echo                   Target="[INSTALLFOLDER]SeisBox.exe"
echo                   WorkingDirectory="INSTALLFOLDER"
echo                   Icon="SeisBoxIcon.exe" /^>
echo         ^<RemoveFolder Id="CleanUpShortCut" Directory="ApplicationProgramsFolder" On="uninstall" /^>
echo         ^<RegistryValue Root="HKCU" Key="Software\%MANUFACTURER%\%PRODUCT_NAME%" Name="installed" Type="integer" Value="1" KeyPath="yes" /^>
echo       ^</Component^>
echo     ^</DirectoryRef^>
echo.
echo     ^<!-- Desktop Shortcut --^>
echo     ^<DirectoryRef Id="DesktopFolder"^>
echo       ^<Component Id="DesktopShortcut" Guid="B2C3D4E5-F6A7-8901-BCDE-F12345678901"^>
echo         ^<Shortcut Id="DesktopShortcutLink"
echo                   Name="%PRODUCT_NAME%"
echo                   Description="Launch %PRODUCT_NAME%"
echo                   Target="[INSTALLFOLDER]SeisBox.exe"
echo                   WorkingDirectory="INSTALLFOLDER"
echo                   Icon="SeisBoxIcon.exe" /^>
echo         ^<RegistryValue Root="HKCU" Key="Software\%MANUFACTURER%\%PRODUCT_NAME%" Name="desktopshortcut" Type="integer" Value="1" KeyPath="yes" /^>
echo       ^</Component^>
echo     ^</DirectoryRef^>
echo.
echo     ^<!-- Add install folder to system PATH --^>
echo     ^<DirectoryRef Id="INSTALLFOLDER"^>
echo       ^<Component Id="AddToPath" Guid="C3D4E5F6-A7B8-9012-CDEF-234567890123"^>
echo         ^<Environment Id="PATH" Name="PATH" Value="[INSTALLFOLDER]" Permanent="no" Part="last" Action="set" System="yes" /^>
echo         ^<RegistryValue Root="HKLM" Key="Software\%MANUFACTURER%\%PRODUCT_NAME%" Name="pathadded" Type="integer" Value="1" KeyPath="yes" /^>
echo       ^</Component^>
echo     ^</DirectoryRef^>
echo.
echo     ^<!-- File Associations: .mseed and .sac --^>
echo     ^<DirectoryRef Id="INSTALLFOLDER"^>
echo       ^<Component Id="FileAssocMseed" Guid="D4E5F6A7-B890-1234-DEFA-567890123456"^>
echo         ^<ProgId Id="SeisBox.MiniSEED" Description="MiniSEED Seismic Data" Icon="SeisBoxIcon.exe"^>
echo           ^<Extension Id="mseed" ContentType="application/x-miniseed"^>
echo             ^<Verb Id="open" Command="Open with SeisBox Picker" TargetFile="[INSTALLFOLDER]seisbox_picker.exe" Argument=""%%%%1"" /^>
echo           ^</Extension^>
echo         ^</ProgId^>
echo         ^<RegistryValue Root="HKLM" Key="Software\%MANUFACTURER%\%PRODUCT_NAME%" Name="mseed_assoc" Type="integer" Value="1" KeyPath="yes" /^>
echo       ^</Component^>
echo       ^<Component Id="FileAssocSac" Guid="E5F6A7B8-9012-3456-EFAB-678901234567"^>
echo         ^<ProgId Id="SeisBox.SAC" Description="SAC Seismic Data" Icon="SeisBoxIcon.exe"^>
echo           ^<Extension Id="sac" ContentType="application/x-sac"^>
echo             ^<Verb Id="open" Command="Open with SeisBox Picker" TargetFile="[INSTALLFOLDER]seisbox_picker.exe" Argument=""%%%%1"" /^>
echo           ^</Extension^>
echo           ^<Extension Id="SAC" ContentType="application/x-sac"^>
echo             ^<Verb Id="open" Command="Open with SeisBox Picker" TargetFile="[INSTALLFOLDER]seisbox_picker.exe" Argument=""%%%%1"" /^>
echo           ^</Extension^>
echo         ^</ProgId^>
echo         ^<RegistryValue Root="HKLM" Key="Software\%MANUFACTURER%\%PRODUCT_NAME%" Name="sac_assoc" Type="integer" Value="1" KeyPath="yes" /^>
echo       ^</Component^>
echo     ^</DirectoryRef^>
echo.
echo   ^</Product^>
echo ^</Wix^>
) > %WXS_FILE%

echo WXS source file generated: %WXS_FILE%

:: ---- Use heat.exe to harvest dist_win directory ----
echo Harvesting %DIST_DIR% with heat.exe...
heat.exe dir "%DIST_DIR%" ^
    -cg ProductComponents ^
    -gg ^
    -scom ^
    -sreg ^
    -sfrag ^
    -srd ^
    -dr INSTALLFOLDER ^
    -var var.DistDir ^
    -out %OBJ_DIR%\FilesFragment.wxs

if %ERRORLEVEL% neq 0 (
    echo Error: heat.exe failed!
    exit /b %ERRORLEVEL%
)

echo Files fragment generated.

:: ---- Compile WXS to WIXOBJ ----
echo Compiling WiX sources...
candle.exe -dDistDir="%DIST_DIR%" ^
    -ext WixUIExtension ^
    -out %OBJ_DIR%\ ^
    %WXS_FILE% ^
    %OBJ_DIR%\FilesFragment.wxs

if %ERRORLEVEL% neq 0 (
    echo Error: candle.exe compilation failed!
    exit /b %ERRORLEVEL%
)

echo Compilation successful.

:: ---- Link WIXOBJ to MSI ----
echo Linking MSI installer...
light.exe -ext WixUIExtension ^
    -out %MSI_FILE% ^
    %OBJ_DIR%\seisbox_installer.wixobj ^
    %OBJ_DIR%\FilesFragment.wixobj

if %ERRORLEVEL% neq 0 (
    echo Error: light.exe linking failed!
    exit /b %ERRORLEVEL%
)

echo.
echo ===================================================
echo  SUCCESS!
echo  MSI Installer: %MSI_FILE%
echo ===================================================
echo.
echo The installer will:
echo  - Install SeisBox to "Program Files\SeisBox"
echo  - Add install folder to system PATH (CLI tools usable from any terminal)
echo  - Register .mseed and .sac files to open with SeisBox Picker
echo  - Create a Start Menu shortcut
echo  - Create a Desktop shortcut
echo  - Support clean uninstall via Add/Remove Programs
echo.

:: Cleanup intermediate files
set /P CLEANUP="Clean up intermediate build files? (y/n) "
if /I "%CLEANUP%"=="y" (
    if exist %OBJ_DIR% rmdir /S /Q %OBJ_DIR%
    if exist %WXS_FILE% del %WXS_FILE%
    echo Intermediate files cleaned.
)
