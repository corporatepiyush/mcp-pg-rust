class McpPostgres < Formula
  desc "MCP server implementation for PostgreSQL"
  homepage "https://github.com/corporatepiyush/mcp-pg-rust"
  url "https://github.com/corporatepiyush/mcp-pg-rust/archive/refs/tags/v3.2.1.tar.gz"
  sha256 "602a7dd48ad8085063bb5a8fb8c01224de3b56cad5bdefa5e113f8b8d3a3586a"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_predicate bin/"mcp-postgres", :exist?
  end
end
