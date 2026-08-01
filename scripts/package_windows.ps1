[CmdletBinding()]
param (
    [string]$Version = "0.1.0",
    [string]$Arch = "x64"
)

$ErrorActionPreference = "Stop"

Write-Host "THOR_DOWNSTREAM_UNSUPPORTED: Windows packaging is gated until it bundles and verifies a Thor-owned Chromium build." -ForegroundColor Red
exit 2

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "📦 Packaging Thor for Windows ($Arch, v$Version)" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# 1. Compile release binaries
Write-Host "🔨 Compiling release binaries..." -ForegroundColor Yellow
cargo build --release --bin Thor --bin thor-labs

$DistDir = "dist"
if (!(Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir | Out-Null
}

$StagingDir = "$DistDir\Thor-windows-$Arch"
if (Test-Path $StagingDir) {
    Remove-Item -Recurse -Force $StagingDir
}
New-Item -ItemType Directory -Path $StagingDir | Out-Null

# Copy Binaries
Copy-Item "target\release\Thor.exe" -Destination "$StagingDir\Thor.exe"
if (Test-Path "target\release\thor-labs.exe") {
    Copy-Item "target\release\thor-labs.exe" -Destination "$StagingDir\thor-labs.exe"
}

# 2. Package Portable ZIP Archive
$ZipName = "Thor-v$Version-windows-$Arch.zip"
$ZipPath = "$DistDir\$ZipName"
Write-Host "🤐 Creating $ZipName..." -ForegroundColor Yellow
if (Test-Path $ZipPath) { Remove-Item -Force $ZipPath }
Compress-Archive -Path "$StagingDir\*" -DestinationPath $ZipPath -Force
Write-Host "✅ Portable ZIP package created: $ZipPath" -ForegroundColor Green

# 3. Create NSIS Installer Script & Executable (.exe)
$InstallerName = "Thor-v$Version-windows-$Arch-setup.exe"
$InstallerPath = "$DistDir\$InstallerName"
$NsiScriptPath = "$DistDir\installer.nsi"

Write-Host "📜 Generating NSIS Script..." -ForegroundColor Yellow
$NsiContent = @"
!define PRODUCT_NAME "Thor"
!define PRODUCT_VERSION "$Version"
!define PRODUCT_PUBLISHER "Thor Contributors"
!define PRODUCT_WEB_SITE "https://thor-browser.org"
!define PRODUCT_DIR_REGKEY "Software\Microsoft\Windows\CurrentVersion\App Paths\Thor.exe"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\`${PRODUCT_NAME}"
!define PRODUCT_UNINST_ROOT_KEY "HKLM"

SetCompressor lzma

Name "`${PRODUCT_NAME} `${PRODUCT_VERSION}"
OutFile "$InstallerName"
InstallDir "`$PROGRAMFILES64\Thor"
InstallDirRegKey HKLM "`${PRODUCT_DIR_REGKEY}" ""
ShowInstDetails show
ShowUnInstDetails show

Section "MainSection" SEC01
  SetOutPath "`$INSTDIR"
  SetOverwrite ifnewer
  File /r "$StagingDir\*"
  
  CreateDirectory "`$SMPROGRAMS\Thor"
  CreateShortCut "`$SMPROGRAMS\Thor\Thor.lnk" "`$INSTDIR\Thor.exe"
  CreateShortCut "`$DESKTOP\Thor.lnk" "`$INSTDIR\Thor.exe"
SectionEnd

Section -Post
  WriteUninstaller "`$INSTDIR\uninst.exe"
  WriteRegStr HKLM "`${PRODUCT_DIR_REGKEY}" "" "`$INSTDIR\Thor.exe"
  WriteRegStr HKLM "`${PRODUCT_UNINST_KEY}" "DisplayName" "`$(^Name)"
  WriteRegStr HKLM "`${PRODUCT_UNINST_KEY}" "UninstallString" "`$INSTDIR\uninst.exe"
  WriteRegStr HKLM "`${PRODUCT_UNINST_KEY}" "DisplayVersion" "`${PRODUCT_VERSION}"
  WriteRegStr HKLM "`${PRODUCT_UNINST_KEY}" "Publisher" "`${PRODUCT_PUBLISHER}"
SectionEnd

Function un.onUninstSuccess
  HideWindow
  MessageBox MB_OK|MB_ICONINFORMATION "`${PRODUCT_NAME} was successfully removed from your computer."
FunctionEnd

Function un.onInit
  MessageBox MB_ICONQUESTION|MB_YESNO|MB_DEFBUTTON2 "Are you sure you want to completely remove `${PRODUCT_NAME} and all of its components?" IDYES +2
  Abort
FunctionEnd

Section Uninstall
  Delete "`$INSTDIR\uninst.exe"
  Delete "`$INSTDIR\Thor.exe"
  Delete "`$INSTDIR\thor-labs.exe"
  Delete "`$DESKTOP\Thor.lnk"
  RMDir /r "`$SMPROGRAMS\Thor"
  RMDir "`$INSTDIR"

  DeleteRegKey HKLM "`${PRODUCT_UNINST_KEY}"
  DeleteRegKey HKLM "`${PRODUCT_DIR_REGKEY}"
  SetAutoClose true
SectionEnd
"@

Set-Content -Path $NsiScriptPath -Value $NsiContent

$Makensis = Get-Command "makensis" -ErrorAction SilentlyContinue
if ($Makensis) {
    Write-Host "🔨 Building NSIS installer using makensis..." -ForegroundColor Yellow
    Push-Location $DistDir
    makensis installer.nsi
    Pop-Location
    Write-Host "✅ NSIS Installer created successfully: $InstallerPath" -ForegroundColor Green
} else {
    Write-Host "⚠️ makensis not found. Creating zip-based executable fallback..." -ForegroundColor Yellow
    Copy-Item $ZipPath -Destination $InstallerPath
}

# 4. Compute SHA256 Checksums
Write-Host "🔒 Computing SHA256 checksums..." -ForegroundColor Yellow
$Hashes = Get-ChildItem -Path $DistDir -File | Where-Object { $_.Name -notmatch "checksums" -and $_.Name -notmatch "installer.nsi" } | Get-FileHash -Algorithm SHA256
$Hashes | ForEach-Object { "$($_.Hash.ToLower())  $($_.Path | Split-Path -Leaf)" } | Out-File -FilePath "$DistDir\checksums.txt" -Encoding utf8 -Append

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "🎉 Thor Windows Packaging Complete!" -ForegroundColor Cyan
Write-Host "Artifacts generated in .\$DistDir\:" -ForegroundColor Cyan
Get-ChildItem -Path $DistDir | Format-Table Name, Length, LastWriteTime
Write-Host "============================================================" -ForegroundColor Cyan
