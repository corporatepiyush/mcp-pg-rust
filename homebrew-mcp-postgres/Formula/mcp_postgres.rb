class McpPostgres < Formula
  desc "MCP server implementation for PostgreSQL"
  homepage "https://github.com/corporatepiyush/mcp-pg-rust"
  url "https://github.com/corporatepiyush/mcp-pg-rust/archive/refs/tags/v1.3.1.tar.gz"
  sha256 "79082f0145d16873b22dd9528a5fa156b9cd5b3a956cd6e7ea01241c4ea4fd1d"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_predicate bin/"mcp-postgres", :exist?
  end
end
