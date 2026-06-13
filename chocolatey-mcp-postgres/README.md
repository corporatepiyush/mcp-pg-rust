# Chocolatey Package for mcp-postgres

This directory contains the Chocolatey package for installing mcp-postgres on Windows.

## Installation

### From Chocolatey Community Repository (When Available)

```powershell
choco install mcp-postgres
```

### From Local Package (Development)

```powershell
choco install mcp-postgres --source="C:\path\to\chocolatey-mcp-postgres"
```

## Usage

After installation, use mcp-postgres from command line:

```powershell
mcp-postgres --database-url "postgres://user:password@localhost:5432/dbname"
mcp-postgres --help
```

## Update

To update to the latest version:

```powershell
choco upgrade mcp-postgres
```

## Uninstall

```powershell
choco uninstall mcp-postgres
```

## Development

### Building the Package

Requires Chocolatey and the NuGet command-line tool.

```powershell
cd chocolatey-mcp-postgres
choco pack
```

### Testing the Package Locally

```powershell
choco install mcp-postgres -s . --force
mcp-postgres --help
choco uninstall mcp-postgres
```

## Package Update Process

When releasing a new version:

1. Update version in `mcp-postgres.nuspec`
2. Get Windows binary SHA256:
   ```powershell
   Get-FileHash .\mcp-postgres-x86_64-pc-windows-gnu.zip -Algorithm SHA256
   ```
3. Update checksum in `tools/chocolateyinstall.ps1`
4. Update download URL with new version tag
5. Run `choco pack`
6. Test locally before submitting to Chocolatey
