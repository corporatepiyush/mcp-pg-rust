# Installation Guide: mcp-postgres

Complete installation instructions for multiple platforms and methods.

---

## Installation Methods

### 1. From crates.io (Recommended for most users)

**Requires**: Rust toolchain (install from https://rustup.rs/)

```bash
# Install latest version
cargo install mcp-postgres

# Verify installation
mcp-postgres --version
```

**Location**: Binary installed to `~/.cargo/bin/mcp-postgres`

**Update**:
```bash
cargo install mcp-postgres --force  # Force reinstall latest
```

---

### 2. From Source (For development)

**Requires**: Rust toolchain, PostgreSQL dev headers, git

```bash
# Clone repository
git clone https://github.com/corporatepiyush/mcp-pg-rust.git
cd mcp-postgres

# Build release binary
cargo build --release

# Binary at: target/release/mcp-postgres

# Optional: install to system
cargo install --path .
```

**For development with hot reload**:
```bash
cargo watch -x "build --release" -c
```

---

### 3. Homebrew (macOS)

**Requires**: Homebrew (install from https://brew.sh/)

```bash
# Tap the formula repository
brew tap corporatepiyush/mcp-postgres

# Install
brew install mcp-postgres

# Verify
mcp-postgres --version

# Update
brew upgrade mcp-postgres

# Uninstall
brew uninstall mcp-postgres
```

**Building from source via Homebrew** (if needed):
```bash
brew install mcp-postgres --HEAD  # Build latest from main
```

---

## Post-Installation Setup

### 1. Verify PostgreSQL Connection

```bash
# Test database connectivity
psql --version                    # Verify psql installed
psql "postgres://user:pass@localhost:5432/mydb" -c "SELECT version();"
```

**Common connection strings**:
- Local (default): `postgres://localhost:5432/postgres`
- With auth: `postgres://user:password@localhost:5432/mydb`
- Unix socket: `postgres:///mydb?host=/var/run/postgresql`

### 2. Create Test Database (Optional)

```bash
# Create a test database for initial testing
psql -U postgres -c "CREATE DATABASE test_mcp;"

# Verify
psql -l | grep test_mcp
```

### 3. Start the Server

```bash
# TCP mode (port 3000)
mcp-postgres --database-url "postgres://localhost:5432/mydb"

# HTTP/2 mode (port 3001)
mcp-postgres --database-url "postgres://localhost:5432/mydb" --http-port 3001

# Stdio mode (for Claude Desktop)
mcp-postgres --database-url "postgres://localhost:5432/mydb" --stdio

# With custom settings
mcp-postgres \
  --database-url "postgres://localhost:5432/mydb" \
  --host 0.0.0.0 \
  --port 5000 \
  --http-port 5001 \
  --min-connections 10 \
  --max-connections 50 \
  --log-level debug
```

---

## Configuration for Claude Desktop

### Setup Steps

**Step 1**: Locate Claude Desktop config file
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
- **Linux**: `~/.config/Claude/claude_desktop_config.json`

**Step 2**: Add mcp-postgres to config

```json
{
  "mcpServers": {
    "postgres": {
      "command": "mcp-postgres",
      "args": [
        "--database-url",
        "postgres://user:password@localhost:5432/mydb",
        "--stdio"
      ]
    }
  }
}
```

**Step 3**: Restart Claude Desktop

The database connection should now be available to Claude.

### Advanced Claude Desktop Config

```json
{
  "mcpServers": {
    "postgres": {
      "command": "mcp-postgres",
      "args": [
        "--database-url",
        "postgres://user:password@localhost:5432/mydb",
        "--stdio",
        "--access-mode",
        "restricted",
        "--log-level",
        "info"
      ]
    }
  }
}
```

---

## Docker Installation

**Create Dockerfile**:
```dockerfile
FROM rust:1.75-slim

WORKDIR /app
RUN apt-get update && apt-get install -y \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install mcp-postgres

ENTRYPOINT ["mcp-postgres"]
```

**Build and run**:
```bash
docker build -t mcp-postgres .

docker run -e DATABASE_URL="postgres://user:pass@host:5432/db" \
  mcp-postgres --stdio
```

---

## Troubleshooting

### Issue: Connection refused

**Solution**: Verify PostgreSQL is running and accessible
```bash
# Check PostgreSQL status (macOS)
brew services list | grep postgres

# Start PostgreSQL if stopped
brew services start postgresql@15

# Test connection
psql "postgres://localhost:5432/postgres" -c "SELECT 1;"
```

### Issue: Command not found: mcp-postgres

**Solution**: Ensure installation directory in PATH
```bash
# Check cargo bin in PATH
echo $PATH | grep cargo/bin

# Add to ~/.bashrc or ~/.zshrc if missing
export PATH="$HOME/.cargo/bin:$PATH"

# Reload shell
source ~/.bashrc  # or source ~/.zshrc
```

### Issue: Permission denied for database

**Solution**: Check PostgreSQL user permissions
```bash
# List users
psql -U postgres -c "\du"

# Grant permissions if needed
psql -U postgres -c "GRANT ALL ON DATABASE mydb TO myuser;"
```

### Issue: SSL/TLS connection error

**Solution**: Use `sslmode=disable` for local development
```bash
mcp-postgres --database-url "postgres://user:pass@localhost:5432/db?sslmode=disable"
```

---

## Verification

### Quick Health Check

```bash
# TCP endpoint
nc -zv 127.0.0.1 3000 && echo "TCP OK"

# HTTP endpoint
curl -s http://127.0.0.1:3001/health | jq '.'

# Test tool call (HTTP)
curl -X POST http://127.0.0.1:3001/rpc \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {
      "name": "show_current_user",
      "arguments": {}
    },
    "id": 1
  }' | jq '.'
```

Expected response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "user": "postgres",
    "database": "mydb",
    "version": "PostgreSQL 15.x..."
  },
  "id": 1
}
```

---

## Uninstallation

### Via cargo
```bash
cargo uninstall mcp-postgres
```

### Via Homebrew
```bash
brew uninstall mcp-postgres
brew untap corporatepiyush/mcp-postgres
```

### From source
```bash
rm -f ~/.cargo/bin/mcp-postgres
# Or remove from system if installed with: cargo install --path .
```

---

## Next Steps

1. **Get Started**: Read [README.md](../README.md) for overview
2. **Quick Testing**: Use [QUICK_TEST.md](./QUICK_TEST.md) for test commands
3. **Setup Environment**: Follow [TEST_SETUP.md](./TEST_SETUP.md) for test database
4. **Integration**: Configure Claude Desktop as shown above
5. **Reference**: Check [guides/INDEX.md](./INDEX.md) for all documentation

---

## Support

For issues:
- Check [README.md](../README.md) troubleshooting section
- Review [SKILLS.md](../SKILLS.md) for SDLC procedures
- Check [guides/](./INDEX.md) for detailed guides
- File issue: https://github.com/corporatepiyush/mcp-pg-rust/issues
