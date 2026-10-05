@echo off
setlocal enabledelayedexpansion

echo ===================================================
echo  SeisBox MSI Installer Builder (Windows) - WiX v5
echo ===================================================
echo.
echo  Requires: WiX Toolset v5 (wix.exe)
echo  Install via: dotnet tool install --global wix --version "5.0.2"
echo ===================================================
echo.

rem Check if dist_win folder exists
set DIST_DIR=dist_win
if not exist %DIST_DIR% (
    echo Error: %DIST_DIR% folder not found!
    echo Please run compile_dist.bat first to build the binaries.
    exit /b 1
)

rem Check if WiX Toolset is available
wix --version >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo WiX Toolset not found or not working.
    echo Please install it using:
    echo dotnet tool install --global wix --version "5.0.2"
    exit /b 1
)

echo WiX Toolset found.
echo Installing/Verifying WiX UI Extension...
wix extension add WixToolset.UI.wixext/5.0.2
echo.

rem ---- Configuration ----
set PRODUCT_NAME=SeisBox
set PRODUCT_VERSION=0.1.1
set MANUFACTURER=Yudha Styawan
set UPGRADE_GUID=7B2E8F4A-1C3D-4E5F-A6B7-8C9D0E1F2A3B
set WXS_FILE=seisbox_installer.wxs
set MSI_FILE=SeisBox_Windows_Installer.msi

rem ---- Check for Custom .ico File ----
set ICON_ID=SeisBoxIcon
set ICON_SRC=%DIST_DIR%\SeisBox.exe
if exist "assets\seisbox_icon.ico" (
    set ICON_ID=SeisBoxIcon
    set ICON_SRC=assets\seisbox_icon.ico
    echo Ditemukan kustom ikon: !ICON_SRC!
) else if exist "%DIST_DIR%\assets\seisbox_icon.ico" (
    set ICON_ID=SeisBoxIcon
    set ICON_SRC=%DIST_DIR%\assets\seisbox_icon.ico
    echo Ditemukan kustom ikon: !ICON_SRC!
) else (
    echo ===================================================
    echo PERINGATAN: File "seisbox_icon.ico" tidak ditemukan di folder assets!
    echo ===================================================
)
echo.

rem ---- Generate LICENSE.rtf from LICENSE text file ----
echo Generating LICENSE.rtf...
if exist LICENSE.rtf del LICENSE.rtf
echo {\rtf1\ansi\deff0{\fonttbl{\f0 Consolas;}} > LICENSE.rtf
echo \f0\fs18 >> LICENSE.rtf
if exist LICENSE (
    for /f "usebackq delims=" %%L in ("LICENSE") do (
        echo %%L\par >> LICENSE.rtf
    )
) else (
    echo SeisBox License\par >> LICENSE.rtf
)
echo } >> LICENSE.rtf
echo LICENSE.rtf generated.
echo.

echo Generating WiX source file...
if exist %WXS_FILE% del %WXS_FILE%

rem ---- Generate WXS (WiX XML Source) Line by Line ----
echo ^<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs" xmlns:ui="http://wixtoolset.org/schemas/v4/wxs/ui"^> >> %WXS_FILE%
echo   ^<Package Name="%PRODUCT_NAME%" Language="1033" Version="%PRODUCT_VERSION%" Manufacturer="%MANUFACTURER%" UpgradeCode="%UPGRADE_GUID%" InstallerVersion="200" Scope="perMachine"^> >> %WXS_FILE%
echo     ^<SummaryInformation Description="%PRODUCT_NAME% %PRODUCT_VERSION% Installer" /^> >> %WXS_FILE%
echo     ^<MajorUpgrade DowngradeErrorMessage="A newer version of [ProductName] is already installed." /^> >> %WXS_FILE%
echo     ^<MediaTemplate EmbedCab="yes" /^> >> %WXS_FILE%
echo     ^<ui:WixUI Id="WixUI_InstallDir" InstallDirectory="INSTALLFOLDER" /^> >> %WXS_FILE%
echo     ^<Icon Id="!ICON_ID!" SourceFile="!ICON_SRC!" /^> >> %WXS_FILE%
echo     ^<Property Id="ARPPRODUCTICON" Value="!ICON_ID!" /^> >> %WXS_FILE%
echo     ^<WixVariable Id="WixUILicenseRtf" Value="LICENSE.rtf" /^> >> %WXS_FILE%
echo     ^<Feature Id="Complete" Title="%PRODUCT_NAME%" Level="1"^> >> %WXS_FILE%
echo       ^<ComponentGroupRef Id="ProductComponents" /^> >> %WXS_FILE%
echo       ^<ComponentRef Id="ProgramMenuShortcut" /^> >> %WXS_FILE%
echo       ^<ComponentRef Id="DesktopShortcut" /^> >> %WXS_FILE%
echo       ^<ComponentRef Id="AddToPath" /^> >> %WXS_FILE%
echo       ^<ComponentRef Id="FileAssocMseed" /^> >> %WXS_FILE%
echo       ^<ComponentRef Id="FileAssocSac" /^> >> %WXS_FILE%
echo       ^<ComponentRef Id="AppFriendlyName" /^> >> %WXS_FILE%
echo     ^</Feature^> >> %WXS_FILE%
echo     ^<StandardDirectory Id="ProgramFilesFolder"^> >> %WXS_FILE%
echo       ^<Directory Id="INSTALLFOLDER" Name="%PRODUCT_NAME%" /^> >> %WXS_FILE%
echo     ^</StandardDirectory^> >> %WXS_FILE%
echo     ^<StandardDirectory Id="ProgramMenuFolder"^> >> %WXS_FILE%
echo       ^<Directory Id="ApplicationProgramsFolder" Name="%PRODUCT_NAME%"^> >> %WXS_FILE%
echo         ^<Component Id="ProgramMenuShortcut" Guid="A1B2C3D4-E5F6-7890-ABCD-EF1234567890"^> >> %WXS_FILE%

for %%F in ("%DIST_DIR%\*.exe") do (
    set "EXE_NAME=%%~nF"
    set "DISP_NAME=!EXE_NAME!"
    set "DISP_NAME=!DISP_NAME:seisbox_picker=SeisBox Picker!"
    set "DISP_NAME=!DISP_NAME:seisbox_stats=SeisBox Stats!"
    set "DISP_NAME=!DISP_NAME:seisbox_hvsr=SeisBox HVSR!"
    set "DISP_NAME=!DISP_NAME:seisbox_cfs=SeisBox CFS!"
    set "DISP_NAME=!DISP_NAME:seisbox_interp=SeisBox Interpolator!"
    set "DISP_NAME=!DISP_NAME:seisbox_fdsn=SeisBox FDSN!"
    set "DISP_NAME=!DISP_NAME:seisbox_isc=SeisBox ISC!"
    set "DISP_NAME=!DISP_NAME:seisbox_inversion=SeisBox Inversion!"
    echo           ^<Shortcut Id="StartMenu_!EXE_NAME!" Name="!DISP_NAME!" Target="[INSTALLFOLDER]%%~nxF" WorkingDirectory="INSTALLFOLDER" Icon="!ICON_ID!" /^> >> %WXS_FILE%
)

echo           ^<RemoveFolder Id="CleanUpShortCut" Directory="ApplicationProgramsFolder" On="uninstall" /^> >> %WXS_FILE%
echo           ^<RegistryValue Root="HKCU" Key="Software\%MANUFACTURER%\%PRODUCT_NAME%" Name="installed" Type="integer" Value="1" KeyPath="yes" /^> >> %WXS_FILE%
echo         ^</Component^> >> %WXS_FILE%
echo       ^</Directory^> >> %WXS_FILE%
echo     ^</StandardDirectory^> >> %WXS_FILE%
echo     ^<StandardDirectory Id="DesktopFolder"^> >> %WXS_FILE%
echo       ^<Component Id="DesktopShortcut" Guid="B2C3D4E5-F6A7-8901-BCDE-F12345678901"^> >> %WXS_FILE%
echo         ^<Shortcut Id="DesktopShortcutLink" Name="%PRODUCT_NAME%" Description="Launch %PRODUCT_NAME%" Target="[INSTALLFOLDER]SeisBox.exe" WorkingDirectory="INSTALLFOLDER" Icon="!ICON_ID!" /^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCU" Key="Software\%MANUFACTURER%\%PRODUCT_NAME%" Name="desktopshortcut" Type="integer" Value="1" KeyPath="yes" /^> >> %WXS_FILE%
echo       ^</Component^> >> %WXS_FILE%
echo     ^</StandardDirectory^> >> %WXS_FILE%
echo     ^<DirectoryRef Id="INSTALLFOLDER"^> >> %WXS_FILE%
echo       ^<Component Id="AddToPath" Guid="C3D4E5F6-A7B8-9012-CDEF-234567890123"^> >> %WXS_FILE%
echo         ^<Environment Id="PATH" Name="PATH" Value="[INSTALLFOLDER]" Permanent="no" Part="last" Action="set" System="yes" /^> >> %WXS_FILE%
echo       ^</Component^> >> %WXS_FILE%
echo     ^</DirectoryRef^> >> %WXS_FILE%
echo     ^<DirectoryRef Id="INSTALLFOLDER"^> >> %WXS_FILE%
echo       ^<Component Id="FileAssocMseed" Guid="D4E5F6A7-B890-1234-DEFA-567890123456"^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key=".mseed" Value="SeisBox.MiniSEED" Type="string" KeyPath="yes" /^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key="SeisBox.MiniSEED" Value="MiniSEED Seismic Data" Type="string" /^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key="SeisBox.MiniSEED\DefaultIcon" Value="[INSTALLFOLDER]assets\seisbox_icon.ico" Type="string" /^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key="SeisBox.MiniSEED\shell\Open with SeisBox Picker" Name="Icon" Value="[INSTALLFOLDER]assets\seisbox_icon.ico" Type="string" /^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key="SeisBox.MiniSEED\shell\Open with SeisBox Picker\command" Value="&quot;[INSTALLFOLDER]seisbox_picker.exe&quot; &quot;%%1&quot;" Type="string" /^> >> %WXS_FILE%
echo       ^</Component^> >> %WXS_FILE%
echo       ^<Component Id="FileAssocSac" Guid="E5F6A7B8-9012-3456-EFAB-678901234567"^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key=".sac" Value="SeisBox.SAC" Type="string" KeyPath="yes" /^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key="SeisBox.SAC" Value="SAC Seismic Data" Type="string" /^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key="SeisBox.SAC\DefaultIcon" Value="[INSTALLFOLDER]assets\seisbox_icon.ico" Type="string" /^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key="SeisBox.SAC\shell\Open with SeisBox Picker" Name="Icon" Value="[INSTALLFOLDER]assets\seisbox_icon.ico" Type="string" /^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key="SeisBox.SAC\shell\Open with SeisBox Picker\command" Value="&quot;[INSTALLFOLDER]seisbox_picker.exe&quot; &quot;%%1&quot;" Type="string" /^> >> %WXS_FILE%
echo       ^</Component^> >> %WXS_FILE%
echo       ^<Component Id="AppFriendlyName" Guid="F6A7B890-1234-5678-ABCD-EF0123456789"^> >> %WXS_FILE%
echo         ^<RegistryValue Root="HKCR" Key="Applications\seisbox_picker.exe" Name="FriendlyAppName" Value="SeisBox Picker" Type="string" KeyPath="yes" /^> >> %WXS_FILE%
echo       ^</Component^> >> %WXS_FILE%
echo     ^</DirectoryRef^> >> %WXS_FILE%
echo     ^<ComponentGroup Id="ProductComponents" Directory="INSTALLFOLDER"^> >> %WXS_FILE%
echo       ^<Files Include="%DIST_DIR%\**" /^> >> %WXS_FILE%
echo     ^</ComponentGroup^> >> %WXS_FILE%
echo   ^</Package^> >> %WXS_FILE%
echo ^</Wix^> >> %WXS_FILE%

echo WXS source file generated: %WXS_FILE%

rem ---- Build MSI with WiX v5 ----
echo Building MSI installer...
wix build -ext WixToolset.UI.wixext -out %MSI_FILE% %WXS_FILE%

if %ERRORLEVEL% neq 0 (
    echo Error: wix build failed!
    exit /b %ERRORLEVEL%
)

echo.
echo ===================================================
echo  SUCCESS!
echo  MSI Installer: %MSI_FILE%
echo ===================================================
echo.

rem Cleanup intermediate files
set /P CLEANUP="Clean up intermediate build files? (y/n) "
if /I "%CLEANUP%"=="y" (
    if exist %WXS_FILE% del %WXS_FILE%
    echo Intermediate files cleaned.
)