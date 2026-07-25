import assert from "node:assert/strict";
import test from "node:test";
import {
  manifestInheritsWorkspaceLints,
  workspaceLintInheritanceFailures,
} from "./cargo-workspace-lints.mjs";

test("recognizes workspace lint inheritance in the lints table", () => {
  assert.equal(
    manifestInheritsWorkspaceLints(`
[package]
name = "example"

[lints]
workspace = true
`),
    true,
  );
});

test("does not mistake another workspace key for lint inheritance", () => {
  assert.equal(
    manifestInheritsWorkspaceLints(`
[package]
version.workspace = true

[dependencies]
serde.workspace = true
`),
    false,
  );
});

test("rejects an explicitly disabled lints table", () => {
  assert.equal(
    manifestInheritsWorkspaceLints(`
[lints]
workspace = false
`),
    false,
  );
});

test("reports every workspace member that does not inherit lints", () => {
  const metadata = {
    workspace_members: ["enabled 0.1.0", "missing 0.1.0"],
    packages: [
      {
        id: "enabled 0.1.0",
        name: "enabled",
        manifest_path: "/repo/enabled/Cargo.toml",
      },
      {
        id: "missing 0.1.0",
        name: "missing",
        manifest_path: "/repo/missing/Cargo.toml",
      },
      {
        id: "external 1.0.0",
        name: "external",
        manifest_path: "/registry/external/Cargo.toml",
      },
    ],
  };
  const manifests = new Map([
    ["/repo/enabled/Cargo.toml", "[lints]\nworkspace = true\n"],
    ["/repo/missing/Cargo.toml", "[package]\nname = \"missing\"\n"],
  ]);

  assert.deepEqual(
    workspaceLintInheritanceFailures(metadata, (manifest) => manifests.get(manifest)),
    [
      "missing must inherit workspace lints with [lints] workspace = true in /repo/missing/Cargo.toml",
    ],
  );
});
