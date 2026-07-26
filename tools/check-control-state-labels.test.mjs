import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { artifactPaths } from "./artifact-paths.mjs";
import { controlStateLabelWarnings } from "./check-control-state-labels.mjs";

function fixture(source) {
  fs.mkdirSync(artifactPaths.tmp, { recursive: true });
  const root = fs.mkdtempSync(path.join(artifactPaths.tmp, "tosklight-control-labels-"));
  const directory = path.join(root, "apps", "example", "src");
  fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(path.join(directory, "Example.tsx"), source);
  return root;
}

test("warns when checkbox and switch semantic state labels are missing", (context) => {
  const root = fixture(`
    export function Example() {
      return <><CheckboxField label="Include"/><SwitchField label="Mode" onLabel="Live"/></>;
    }
  `);
  context.after(() => fs.rmSync(root, { recursive: true, force: true }));
  assert.deepEqual(controlStateLabelWarnings(root), [
    "apps/example/src/Example.tsx:3 CheckboxField is missing stateLabel",
    "apps/example/src/Example.tsx:3 SwitchField is missing offLabel",
  ]);
});

test("accepts an explicit stable checkbox label and both switch values", (context) => {
  const root = fixture(`
    export function Example() {
      return <><CheckboxField label="Include" stateLabel="Include in import"/>
        <SwitchField label="Mode" offLabel="Manual" onLabel="Automatic"/></>;
    }
  `);
  context.after(() => fs.rmSync(root, { recursive: true, force: true }));
  assert.deepEqual(controlStateLabelWarnings(root), []);
});
