[CmdletBinding()]
param(
  [ValidateSet("x64", "arm64")]
  [string] $Architecture = $(if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" })
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$SourcePath = Join-Path $ScriptDir "shortcut-aumid-helper.cpp"
$OutputDirectory = Join-Path $ProjectDir "target\fission\windows\shortcut-aumid\$Architecture"
$OutputPath = Join-Path $OutputDirectory "fission-shortcut-aumid.exe"
$ObjectPath = Join-Path $OutputDirectory "fission-shortcut-aumid.obj"

if (-not (Test-Path $SourcePath -PathType Leaf)) {
  throw "The shortcut AUMID helper source was not found at $SourcePath."
}

$VsWhere = Get-Command vswhere.exe -ErrorAction SilentlyContinue
if (-not $VsWhere -and ${env:ProgramFiles(x86)}) {
  $BundledVsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $BundledVsWhere -PathType Leaf) {
    $VsWhere = Get-Item $BundledVsWhere
  }
}
if (-not $VsWhere) {
  throw "vswhere.exe was not found. Install Visual Studio Build Tools with the target C++ toolchain."
}
$VsWherePath = if ($VsWhere -is [System.IO.FileInfo]) {
  $VsWhere.FullName
} else {
  $VsWhere.Source
}

$RequiredComponent = if ($Architecture -eq "arm64") {
  "Microsoft.VisualStudio.Component.VC.Tools.ARM64"
} else {
  "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"
}
$Installation = & $VsWherePath -latest -products * -requires $RequiredComponent -property installationPath
if (-not $Installation) {
  throw "Visual Studio Build Tools with component $RequiredComponent were not found for $Architecture."
}
$VsDevCmd = Join-Path $Installation "Common7\Tools\VsDevCmd.bat"
if (-not (Test-Path $VsDevCmd -PathType Leaf)) {
  throw "VsDevCmd.bat was not found at $VsDevCmd."
}

$DeveloperCommand = "call `"$VsDevCmd`" -no_logo -arch=$Architecture -host_arch=amd64 && set"
$EnvironmentLines = & $env:ComSpec /d /c $DeveloperCommand
if ($LASTEXITCODE -ne 0) {
  throw "Visual Studio failed to initialize the $Architecture C++ build environment."
}
foreach ($Line in $EnvironmentLines) {
  $Separator = $Line.IndexOf("=")
  if ($Separator -gt 0) {
    $Name = $Line.Substring(0, $Separator)
    $Value = $Line.Substring($Separator + 1)
    [Environment]::SetEnvironmentVariable($Name, $Value, "Process")
  }
}

$Compiler = Get-Command cl.exe -ErrorAction SilentlyContinue
if (-not $Compiler) {
  throw "cl.exe was not available after initializing the $Architecture C++ build environment."
}

New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$CompileArguments = @(
  "/nologo",
  "/EHsc",
  "/MT",
  "/DUNICODE",
  "/D_UNICODE",
  "/Fo$ObjectPath",
  "/Fe$OutputPath",
  $SourcePath,
  "/link",
  "ole32.lib",
  "shell32.lib",
  "propsys.lib"
)
& $Compiler.Source @CompileArguments | Out-Host
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $OutputPath -PathType Leaf)) {
  throw "The $Architecture shortcut AUMID helper build failed."
}

Write-Output $OutputPath
