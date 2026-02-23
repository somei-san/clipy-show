class CliipShow < Formula
  desc "Show copied clipboard text as a HUD on macOS"
  homepage "https://github.com/somei-san/cliip-show"
  url "https://github.com/somei-san/cliip-show/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "2cac71f71a7fe3399f335b1997d8e933accc67d6dcc47a1dfd7fa9a0b723027d"
  license "MIT"
  head "https://github.com/somei-san/cliip-show.git", branch: "main"

  depends_on :macos
  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  def caveats
    <<~EOS
      To launch cliip-show now and restart at login:
        brew services start cliip-show

      To stop:
        brew services stop cliip-show
    EOS
  end

  service do
    run [opt_bin/"cliip-show"]
    keep_alive true
    run_at_load true
    process_type :interactive
    log_path var/"log/cliip-show.log"
    error_log_path var/"log/cliip-show.log"
  end

  test do
    assert_equal version.to_s, shell_output("#{bin}/cliip-show --version").strip
  end
end
