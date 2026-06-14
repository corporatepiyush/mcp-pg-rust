class McpPostgres < Formula
  desc "MCP server implementation for PostgreSQL"
  homepage "https://github.com/corporatepiyush/mcp-pg-rust"
  url "https://github.com/corporatepiyush/mcp-pg-rust/archive/refs/tags/v3.1.0.tar.gz"
  sha256 "b17d9942e42d21c0ab14e9c2ba9bc37507643813c6a4d22a735e739455ce4771"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_predicate bin/"mcp-postgres", :exist?
  end
end
