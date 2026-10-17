# Template for a tap formula (zyvorai/homebrew-tap). Fill in url and sha256 from a published release.
class Macora < Formula
  desc "Explainable network cockpit for macOS: CLI, TUI and diagnostics"
  homepage "https://github.com/zyvorai/zyvor-macora"
  url "https://github.com/zyvorai/zyvor-macora/archive/refs/tags/v0.3.0.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_TARBALL_SHA256"
  license "Apache-2.0"
  depends_on "rust" => :build
  depends_on :macos

  def install
    system "cargo", "install", *std_cargo_args
    man1.install "man/macora.1"
    zsh_completion.install "completions/_macora"
    bash_completion.install "completions/macora.bash" => "macora"
    fish_completion.install "completions/macora.fish"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/macora --version")
    assert_match "subnet", shell_output("#{bin}/macora completions bash")
  end
end
