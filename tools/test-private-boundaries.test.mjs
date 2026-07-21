import assert from "node:assert/strict";
import test from "node:test";
import { scanPrivateTestBoundaries } from "./test-private-boundaries.mjs";

test("rejects session storage, Tauri internals, and raw Light events", () => {
  const failures = scanPrivateTestBoundaries([{
    file: "tests/private.spec.ts",
    source: `
      localStorage.getItem("light.primary-session");
      window.__TAURI_INTERNALS__ = {};
      window.dispatchEvent(new CustomEvent("light:desk-action"));
    `,
  }]);
  assert.deepEqual(failures, [
    "tests/private.spec.ts reads the private primary-session storage record",
    "tests/private.spec.ts fabricates private Tauri runtime internals",
    "tests/private.spec.ts dispatches private light:desk-action instead of a public adapter",
  ]);
});

test("retains only the named hosted-picker compatibility control", () => {
  assert.deepEqual(scanPrivateTestBoundaries([{
    file: "tests/16-file-manager.spec.ts",
    source: `window.dispatchEvent(new CustomEvent("light:open-file-manager-picker"));`,
  }]), []);
  assert.deepEqual(scanPrivateTestBoundaries([{
    file: "tests/another.spec.ts",
    source: `window.dispatchEvent(new Event("light:open-file-manager-picker"));`,
  }]), [
    "tests/another.spec.ts dispatches private light:open-file-manager-picker instead of a public adapter",
  ]);
});

test("allows ordinary DOM interaction and closed bench bindings", () => {
  assert.deepEqual(scanPrivateTestBoundaries([{
    file: "tests/public.spec.ts",
    source: `
      control.dispatchEvent(new PointerEvent("pointerdown"));
      await desk.session();
      await desk.enableControllableDesktop();
    `,
  }]), []);
});
