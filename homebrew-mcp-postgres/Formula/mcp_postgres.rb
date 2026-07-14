class McpPostgres < Formula
  desc "MCP server implementation for PostgreSQL"
  homepage "https://github.com/corporatepiyush/mcp-pg-rust"
  url "https://github.com/corporatepiyush/mcp-pg-rust/archive/refs/tags/v4.0.6.tar.gz"
  sha256 "7a87ed7e7baddcee1f3007384fe6f08fd03ea29e660595fe30ccf1e8f211ebba"
  license "Apache-2.0"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_predicate bin/"mcp-postgres", :exist?
  end
end
