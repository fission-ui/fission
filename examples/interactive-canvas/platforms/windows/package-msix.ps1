$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$Profile = if ($env:WINDOWS_PROFILE) { $env:WINDOWS_PROFILE } else { "debug" }
$CargoProfileArg = if ($Profile -eq "release") { @("--release") } else { @() }
$ExecutableName = if ($env:WINDOWS_EXECUTABLE_NAME) { $env:WINDOWS_EXECUTABLE_NAME } else { "interactive-canvas-example.exe" }
$BinaryPath = if ($env:WINDOWS_BINARY) { $env:WINDOWS_BINARY } else { Join-Path $ProjectDir "target\$Profile\$ExecutableName" }
$OutRoot = Join-Path $ProjectDir "target\fission\windows\msix"
$LayoutDir = Join-Path $OutRoot "layout"
$AppDir = Join-Path $LayoutDir "VFS\ProgramFilesX64\interactive-canvas"
$AssetsDir = Join-Path $LayoutDir "Assets"
$MsixPath = Join-Path $OutRoot "dev.fission.interactive.canvas-$Profile.msix"

if (-not $env:WINDOWS_BINARY) {
  cargo build @CargoProfileArg --manifest-path (Join-Path $ProjectDir "Cargo.toml")
}
if (-not (Test-Path $BinaryPath)) {
  throw "Windows executable was not found at $BinaryPath. Set WINDOWS_BINARY or WINDOWS_EXECUTABLE_NAME if the crate name changed."
}
$MakeAppx = Get-Command makeappx -ErrorAction SilentlyContinue
if (-not $MakeAppx) {
  throw "makeappx was not found. Install Windows SDK MSIX packaging tools and ensure makeappx is on PATH."
}

Remove-Item -Recurse -Force $LayoutDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $AppDir, $AssetsDir | Out-Null
Copy-Item $BinaryPath (Join-Path $AppDir $ExecutableName) -Force
if ($env:FISSION_WINDOWS_NATIVE_PRODUCTS_MANIFEST) {
  $NativeManifest = Get-Content -Raw $env:FISSION_WINDOWS_NATIVE_PRODUCTS_MANIFEST | ConvertFrom-Json
  foreach ($Product in $NativeManifest.products) {
    if ($Product.kind -eq "driver-package") {
      throw "MSIX native product manifest must not contain driver package $($Product.name)."
    }
    $NativeDestination = Join-Path $AppDir $Product.destination
    $NativeParent = Split-Path -Parent $NativeDestination
    New-Item -ItemType Directory -Force $NativeParent | Out-Null
    if (Test-Path $Product.source -PathType Container) {
      Copy-Item $Product.source $NativeDestination -Recurse -Force
    } else {
      Copy-Item $Product.source $NativeDestination -Force
    }
  }
}
Copy-Item (Join-Path $ScriptDir "Package.appxmanifest") (Join-Path $LayoutDir "AppxManifest.xml") -Force

$IconSource = if ($env:WINDOWS_APP_ICON) { $env:WINDOWS_APP_ICON } else { Join-Path $ProjectDir "assets\app-icon.png" }
if (Test-Path $IconSource) {
  Copy-Item $IconSource (Join-Path $AssetsDir "StoreLogo.png") -Force
  Copy-Item $IconSource (Join-Path $AssetsDir "Square44x44Logo.png") -Force
  Copy-Item $IconSource (Join-Path $AssetsDir "Square150x150Logo.png") -Force
}

& $MakeAppx.Source pack /d $LayoutDir /p $MsixPath /overwrite | Out-Host

$Certificate = $env:WINDOWS_CERTIFICATE
$TempCertificate = $null
try {
  if (-not $Certificate -and $env:WINDOWS_CERTIFICATE_BASE64) {
    $TempCertificate = Join-Path ([System.IO.Path]::GetTempPath()) ("fission-windows-cert-" + [System.Guid]::NewGuid().ToString() + ".pfx")
    [System.IO.File]::WriteAllBytes($TempCertificate, [System.Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_BASE64))
    $Certificate = $TempCertificate
  }
  $Thumbprint = $env:WINDOWS_CERTIFICATE_THUMBPRINT
  if ($Certificate -or $Thumbprint) {
    $SignTool = Get-Command signtool -ErrorAction SilentlyContinue
    if (-not $SignTool) {
      throw "signtool was not found. Install Windows SDK signing tools or set WINDOWS_SKIP_SIGNING=1 for unsigned local packages."
    }
    $SignArgs = @("sign", "/fd", "SHA256")
    if ($Certificate) {
      $SignArgs += @("/f", $Certificate)
      if ($env:WINDOWS_CERTIFICATE_PASSWORD) { $SignArgs += @("/p", $env:WINDOWS_CERTIFICATE_PASSWORD) }
    } else {
      $SignArgs += @("/sha1", $Thumbprint)
    }
    $SignArgs += $MsixPath
    & $SignTool.Source @SignArgs | Out-Host
  } elseif ($Profile -eq "release" -and $env:WINDOWS_SKIP_SIGNING -ne "1") {
    throw "Release MSIX packaging requires WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_BASE64, or WINDOWS_CERTIFICATE_THUMBPRINT from a secure secret source. Set WINDOWS_SKIP_SIGNING=1 only for local unsigned validation."
  }
} finally {
  if ($TempCertificate) { Remove-Item -Force $TempCertificate -ErrorAction SilentlyContinue }
}

Write-Output $MsixPath
