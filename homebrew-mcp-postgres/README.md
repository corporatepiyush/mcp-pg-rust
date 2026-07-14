# Homebrew Tap for mcp-postgres

This directory contains the Homebrew formula for installing mcp-postgres.

## Installation

### Option 1: From Local Repository (Development)

If you have cloned the mcp-postgres repository:

```bash
brew tap corporatepiyush/mcp-postgres /path/to/mcp-postgres/homebrew-mcp-postgres
brew install mcp-postgres
```

### Option 2: From Separate Tap Repository (When Available)

Once published to a separate Homebrew tap:

```bash
brew tap corporatepiyush/mcp-postgres
brew install mcp-postgres
```

### Option 3: Direct Install (from source)

```bash
cargo install --git https://github.com/corporatepiyush/mcp-pg-rust
```

## Update

To update to the latest version:

```bash
brew upgrade mcp-postgres
```

## Uninstall

```bash
brew uninstall mcp-postgres
brew untap corporatepiyush/mcp-postgres
```

## Development

To test the formula locally:

```bash
cd homebrew-mcp-postgres
brew tap corporatepiyush/mcp-postgres .
brew install corporatepiyush/mcp-postgres/mcp_postgres
```
