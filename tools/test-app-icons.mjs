#!/usr/bin/env node

import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { repositoryRoot } from "./artifact-paths.mjs";

const requiredIcons = [
  "icons/32x32.png",
  "icons/128x128.png",
  "icons/128x128@2x.png",
  "icons/icon.icns",
  "icons/icon.ico",
];

const applications = [
  path.join(repositoryRoot, "apps/control-ui/src-tauri"),
  path.join(repositoryRoot, "apps/hardware-controls/src-tauri"),
];

for (const application of applications) {
  const config = JSON.parse(fs.readFileSync(path.join(application, "tauri.conf.json"), "utf8"));
  assert.deepEqual(config.bundle?.icon, requiredIcons, `${application} must declare the complete desktop icon set`);
  for (const icon of requiredIcons) {
    const iconPath = path.join(application, icon);
    assert.ok(fs.statSync(iconPath).size > 0, `${iconPath} must exist and not be empty`);
  }
}

const digest = (file) => crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
const controlIcon = path.join(applications[0], "icons/icon.png");
const hardwareIcon = path.join(applications[1], "icons/icon.png");
assert.notEqual(digest(controlIcon), digest(hardwareIcon), "ToskLight and Hardware Controls must use distinct icons");

console.log("ToskLight desktop icon configuration is complete and application-specific.");
