$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$Profile = if ($env:WINDOWS_PROFILE) { $env:WINDOWS_PROFILE } else { "debug" }
$CargoProfileArg = if ($Profile -eq "release") { @("--release") } else { @() }
$ExecutableName = if ($env:WINDOWS_EXECUTABLE_NAME) { $env:WINDOWS_EXECUTABLE_NAME } else { "field-inspector.exe" }
$BinaryPath = if ($env:WINDOWS_BINARY) { $env:WINDOWS_BINARY } else { Join-Path $ProjectDir "target\$Profile\$ExecutableName" }
$OutRoot = Join-Path $ProjectDir "target\fission\windows\msi"
$MsiPath = Join-Path $OutRoot "field-inspector-$Profile.msi"
$Version = if ($env:WINDOWS_MSI_VERSION) { $env:WINDOWS_MSI_VERSION } else { "0.1.0" }
$UpgradeCode = if ($env:WINDOWS_MSI_UPGRADE_CODE) { $env:WINDOWS_MSI_UPGRADE_CODE } else { "ecabf965-aa73-453b-a4f1-1155353599c4" }

if (-not $env:WINDOWS_BINARY) {
  cargo build @CargoProfileArg --manifest-path (Join-Path $ProjectDir "Cargo.toml")
}
if (-not (Test-Path $BinaryPath)) {
  throw "Windows executable was not found at $BinaryPath. Set WINDOWS_BINARY or WINDOWS_EXECUTABLE_NAME if the crate name changed."
}
New-Item -ItemType Directory -Force $OutRoot | Out-Null

$Wix = Get-Command wix -ErrorAction SilentlyContinue
$Candle = Get-Command candle -ErrorAction SilentlyContinue
$Light = Get-Command light -ErrorAction SilentlyContinue
if ($Wix) {
  $WxsPath = Join-Path $OutRoot "package.wxs"
  @"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="field-inspector" Manufacturer="Fission Developer" Version="$Version" UpgradeCode="$UpgradeCode" Scope="perMachine">
    <MajorUpgrade DowngradeErrorMessage="A newer version of field-inspector is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <StandardDirectory Id="ProgramFiles6432Folder">
      <Directory Id="INSTALLFOLDER" Name="field-inspector">
        <Component Id="MainExecutable" Guid="*">
          <File Id="AppExe" Source="$BinaryPath" KeyPath="yes" />
        </Component>
      </Directory>
    </StandardDirectory>
    <Feature Id="MainFeature" Title="field-inspector" Level="1">
      <ComponentRef Id="MainExecutable" />
    </Feature>
  </Package>
</Wix>
"@ | Set-Content -Encoding UTF8 $WxsPath
  & $Wix.Source build $WxsPath -o $MsiPath | Out-Host
} elseif ($Candle -and $Light) {
  $WxsPath = Join-Path $OutRoot "package-wix3.wxs"
  $WixObj = Join-Path $OutRoot "package.wixobj"
  @"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="field-inspector" Language="1033" Version="$Version" Manufacturer="Fission Developer" UpgradeCode="$UpgradeCode">
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perMachine" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of field-inspector is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLFOLDER" Name="field-inspector">
          <Component Id="MainExecutable" Guid="*">
            <File Id="AppExe" Source="$BinaryPath" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="MainFeature" Title="field-inspector" Level="1">
      <ComponentRef Id="MainExecutable" />
    </Feature>
  </Product>
</Wix>
"@ | Set-Content -Encoding UTF8 $WxsPath
  & $Candle.Source -nologo -arch x64 -out $WixObj $WxsPath | Out-Host
  & $Light.Source -nologo -out $MsiPath $WixObj | Out-Host
} else {
  throw "WiX was not found. Install WiX Toolset (`wix`) or WiX 3 (`candle` and `light`) to package an MSI."
}

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
    $SignArgs += $MsiPath
    & $SignTool.Source @SignArgs | Out-Host
  } elseif ($Profile -eq "release" -and $env:WINDOWS_SKIP_SIGNING -ne "1") {
    throw "Release MSI packaging requires WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_BASE64, or WINDOWS_CERTIFICATE_THUMBPRINT from a secure secret source. Set WINDOWS_SKIP_SIGNING=1 only for local unsigned validation."
  }
} finally {
  if ($TempCertificate) { Remove-Item -Force $TempCertificate -ErrorAction SilentlyContinue }
}

Write-Output $MsiPath
