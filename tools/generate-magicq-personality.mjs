#!/usr/bin/env node
// The Rust application exports the exact runtime download bytes from canonical ChannelSpec data.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("..", import.meta.url));
const args = ["run", "--quiet", "-p", "media-application", "--example", "console-personalities", "--", "assets/media-personalities/magicq"];
if (process.argv.includes("--check")) args.push("--check");
const result = spawnSync("cargo", args, { cwd: root, stdio: "inherit" });
if (result.error) throw result.error;
process.exit(result.status ?? 1);
