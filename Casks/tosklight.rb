# frozen_string_literal: true

cask "tosklight" do
  version "1.9.1"
  sha256 "94a91f237379b64b4aea61db356ee6282e761cfd774e761d53d1bcfdab0338fa"

  url "https://github.com/kellertobias/tosklight/releases/download/v#{version}/tosklight-bundle-macos_arm64.zip"
  name "ToskLight"
  name "ToskLight Control"
  name "ToskLight Architect"
  name "ToskLight Pixel"
  desc "Lighting control desk, CAD and visualizer, and media server"
  homepage "https://github.com/kellertobias/tosklight"

  auto_updates true
  depends_on arch: :arm64
  depends_on formula: "ffmpeg"
  depends_on :macos

  app "tosklight-bundle-macos_arm64/tosklight-desk-macos_arm64.app", target: "ToskLight Control.app"
  app "tosklight-bundle-macos_arm64/tosklight-architect-macos_arm64.app", target: "ToskLight Architect.app"
  app "tosklight-bundle-macos_arm64/tosklight-media-macos_arm64.app", target: "ToskLight Pixel.app"
  app "tosklight-bundle-macos_arm64/ToskLight Launcher.app"

  preflight_steps do
    run "/bin/bash",
        args:         [
          "{{staged_path}}/tosklight-bundle-macos_arm64/sign-macos-apps-locally.sh",
          "{{staged_path}}/tosklight-bundle-macos_arm64",
        ],
        print_stdout: true
  end

  caveats <<~EOS
    Installs Control, Architect (including its visualizer), and Pixel.
    Testing builds are signed locally and verified during installation.
    Valid Developer ID signatures are preserved. If macOS blocks an app, verify
    its source, then use System Settings > Privacy & Security > Open Anyway.
    FFmpeg and ffprobe are installed for Pixel media import.
  EOS
end
