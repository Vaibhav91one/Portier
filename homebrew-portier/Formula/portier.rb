class Portier < Formula
  desc "Port conflict detection and resolution CLI tool"
  homepage "https://github.com/Vaibhav91one/Portier"
  version "0.1.0"
  license "MIT"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/Vaibhav91one/Portier/releases/download/v#{version}/portier-aarch64-apple-darwin.tar.gz"
    sha256 "d380c66f8c9bf87b111749b68c05c05037a3d94062a5d5849385f4c063246888"
  elsif OS.mac? && Hardware::CPU.intel?
    url "https://github.com/Vaibhav91one/Portier/releases/download/v#{version}/portier-x86_64-apple-darwin.tar.gz"
    sha256 "8c357a22b8bd5eca7bcb1762c8a0abaf99b6ba689b16d575ce1b568351cf8c30"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/Vaibhav91one/Portier/releases/download/v#{version}/portier-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "fc912e48e7f079df2a2c1fac5ff2149b3e3d4b01cf621b88506d9592bed454ee"
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
