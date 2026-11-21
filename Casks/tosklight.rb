# frozen_string_literal: true

cask "tosklight" do
  version "1.7.0"
  sha256 "8f38c3422b870f1ef05a8424cedc71d9b778b3f2bbc882aa8a711158579152b0"

  url "https://github.com/kellertobias/tosklight/releases/download/v#{version}/tosklight-bundle-macos_arm64.zip"
  name "ToskLight"
  name "ToskLight Control"
  name "ToskLight Architect"
  name "ToskLight Pixel"
  desc "Lighting control desk, CAD and visualizer, and media server"
  homepage "https://github.com/kellertobias/tosklight"

  depends_on arch: :arm64
  depends_on formula: "ffmpeg"
  depends_on :macos

  app "tosklight-bundle-macos_arm64/tosklight-desk-macos_arm64.app", target: "ToskLight Control.app"
  app "tosklight-bundle-macos_arm64/tosklight-architect-macos_arm64.app", target: "ToskLight Architect.app"
  app "tosklight-bundle-macos_arm64/tosklight-media-macos_arm64.app", target: "ToskLight Pixel.app"

  caveats <<~EOS
    Installs Control, Architect (including its visualizer), and Pixel.
    Testing builds use ad-hoc signatures. If macOS blocks an app, verify
    its source, then use System Settings > Privacy & Security > Open Anyway.
    FFmpeg and ffprobe are installed for Pixel media import.
  EOS
end
