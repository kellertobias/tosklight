#!/usr/bin/env node
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

export const repository = "kellertobias/tosklight";
export const assetName = "tosklight-bundle-macos_arm64.zip";
const bundle = "tosklight-bundle-macos_arm64";

export function renderCask(version, checksum, { launcher = false } = {}) {
	if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("Expected a stable semantic version");
	if (!/^[a-f0-9]{64}$/.test(checksum)) throw new Error("Expected a SHA-256 checksum");
	return `# frozen_string_literal: true

cask "tosklight" do
  version "${version}"
  sha256 "${checksum}"

  url "https://github.com/${repository}/releases/download/v#{version}/${assetName}"
  name "ToskLight"
  name "ToskLight Control"
  name "ToskLight Architect"
  name "ToskLight Pixel"
  desc "Lighting control desk, CAD and visualizer, and media server"
  homepage "https://github.com/${repository}"
${launcher ? "\n  auto_updates true" : ""}
  depends_on arch: :arm64
  depends_on formula: "ffmpeg"
  depends_on :macos

  app "${bundle}/tosklight-desk-macos_arm64.app", target: "ToskLight Control.app"
  app "${bundle}/tosklight-architect-macos_arm64.app", target: "ToskLight Architect.app"
  app "${bundle}/tosklight-media-macos_arm64.app", target: "ToskLight Pixel.app"
${launcher ? `  app "${bundle}/ToskLight Launcher.app"\n` : ""}
  preflight_steps do
    run "/bin/bash",
        args:         [
          "{{staged_path}}/${bundle}/sign-macos-apps-locally.sh",
          "{{staged_path}}/${bundle}",
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
`;
}

export function verifyBundle(archive, expectedChecksum, entries) {
	const checksum = createHash("sha256").update(fs.readFileSync(archive)).digest("hex");
	if (checksum !== expectedChecksum) throw new Error("macOS bundle SHA-256 does not match GitHub's asset digest");
	for (const relative of [
		"sign-macos-apps-locally.sh",
		"tosklight-desk-macos_arm64.app/Contents/MacOS/light-desktop",
		"tosklight-desk-macos_arm64.app/Contents/MacOS/light-headless",
		"tosklight-architect-macos_arm64.app/Contents/MacOS/ToskLight Architect",
		"tosklight-architect-macos_arm64.app/Contents/MacOS/viz-renderer",
		"tosklight-architect-macos_arm64.app/Contents/MacOS/light-headless",
		"tosklight-media-macos_arm64.app/Contents/MacOS/ToskLight Pixel",
	]) {
		if (!entries.includes(`${bundle}/${relative}`)) throw new Error(`macOS bundle is missing ${relative}`);
	}
	return checksum;
}

function run(command, args) {
	const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
	if (result.status !== 0) throw new Error(result.error?.message || result.stderr || `${command} failed`);
	return result.stdout;
}

function main() {
	const args = process.argv.slice(2);
	if (args.length > 1 || (args[0] && !/^v?\d+\.\d+\.\d+$/.test(args[0]))) {
		throw new Error("usage: node tools/update-homebrew-cask.mjs [VERSION] (requires authenticated gh)");
	}
	const release = JSON.parse(run("gh", ["release", "view", ...(args[0] ? [`v${args[0].replace(/^v/, "")}`] : []),
		"--repo", repository, "--json", "tagName,assets,isDraft,isPrerelease"]));
	if (release.isDraft || release.isPrerelease) throw new Error("Homebrew requires a published stable release");
	const version = release.tagName.replace(/^v/, "");
	const asset = release.assets.find((entry) => entry.name === assetName);
	if (!asset?.digest?.match(/^sha256:[a-f0-9]{64}$/)) throw new Error("macOS release asset has no SHA-256 digest");
	const expected = asset.digest.slice("sha256:".length);
	// Validate release-controlled values before using them in commands or Ruby source.
	renderCask(version, expected);
	fs.mkdirSync(artifactPaths.tmp, { recursive: true });
	const scratch = fs.mkdtempSync(path.join(artifactPaths.tmp, "homebrew-cask-"));
	try {
		console.log(`Downloading and verifying ${release.tagName}/${assetName}...`);
		run("gh", ["release", "download", release.tagName, "--repo", repository,
			"--pattern", assetName, "--dir", scratch]);
		const archive = path.join(scratch, assetName);
		const entries = run("unzip", ["-Z1", archive]).split(/\r?\n/);
		const checksum = verifyBundle(archive, expected, entries);
		const destination = path.join(repositoryRoot, "Casks", "tosklight.rb");
		fs.mkdirSync(path.dirname(destination), { recursive: true });
		const launcher = entries.includes(`${bundle}/ToskLight Launcher.app/Contents/MacOS/ToskLight Launcher`);
		fs.writeFileSync(destination, renderCask(version, checksum, { launcher }));
		console.log(`Updated ${destination} to ${version}. Review and commit through Forgejo.`);
	} finally {
		fs.rmSync(scratch, { recursive: true, force: true });
	}
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	try { main(); } catch (error) {
		console.error(`error: ${error.message}`);
		process.exitCode = 1;
	}
}
