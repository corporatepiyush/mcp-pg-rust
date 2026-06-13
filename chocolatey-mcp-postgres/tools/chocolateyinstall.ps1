$ErrorActionPreference = 'Stop'

$packageName = 'mcp-postgres'
$version = '1.3.1'
$url = 'https://github.com/corporatepiyush/mcp-pg-rust/releases/download/v1.3.1/mcp-postgres-x86_64-pc-windows-gnu.zip'
$checksum = 'CHECKSUM_HERE'
$checksumType = 'sha256'
$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

# Download and extract
Install-ChocolateyZipPackage -PackageName $packageName `
  -Url $url `
  -UnzipLocation $toolsDir `
  -Checksum $checksum `
  -ChecksumType $checksumType

# Create batch wrapper for PATH
$binDir = Join-Path $toolsDir 'bin'
$batFile = Join-Path $binDir "$packageName.bat"

if (-not (Test-Path $binDir)) {
  New-Item -ItemType Directory -Path $binDir -Force | Out-Null
}

# Write batch file to make executable available in PATH
@"
@echo off
`"%~dp0..\mcp-postgres.exe`" %*
"@ | Set-Content -Path $batFile -Encoding ASCII -Force

Write-ChocolateySuccess $packageName
