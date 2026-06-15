class McpPostgres < Formula
  desc "MCP server implementation for PostgreSQL"
  homepage "https://github.com/corporatepiyush/mcp-pg-rust"
  url "https://github.com/corporatepiyush/mcp-pg-rust/archive/refs/tags/v4.0.0.tar.gz"
  sha256 "6fc8cde882711a413f0ae90e52cd4554ae1df40c88b6a7456b59516f1319bb64"
  license "Apache-2.0"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_predicate bin/"mcp-postgres", :exist?
  end
end
