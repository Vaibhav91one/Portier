class Portier < Formula
  desc "Port conflict detection and resolution CLI tool"
  homepage "https://github.com/Vaibhav91one/Portier"
  version "0.1.0"
  license "MIT"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/Vaibhav91one/Portier/releases/download/v#{version}/portier-aarch64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  elsif OS.mac? && Hardware::CPU.intel?
    url "https://github.com/Vaibhav91one/Portier/releases/download/v#{version}/portier-x86_64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/Vaibhav91one/Portier/releases/download/v#{version}/portier-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  else
    odie "Unsupported platform: #{OS.kernel_name} #{Hardware::CPU.arch}"
  end

  def install
    bin.install "portier"
  end

  test do
    system "#{bin}/portier", "--version"
  end
end
