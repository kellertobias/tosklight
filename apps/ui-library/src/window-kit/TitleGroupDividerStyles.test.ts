/// <reference types="vite/client" />

import { describe, expect, it } from "vitest";
import modalStyles from "../styles/input.css?raw";
import tokens from "../styles/tokens.css?raw";
import sharedStyles from "../styles/window-kit.css?raw";

/** The declaration block of the first rule whose selector list contains `selector`. */
function ruleContaining(source: string, selector: string): string | undefined {
  for (const match of source.matchAll(/([^{}]+)\{([^}]*)\}/g)) {
    if (match[1].includes(selector)) return match[2];
  }
  return undefined;
}

describe("title action group boundaries", () => {
  it("draws the divider highlight in the palette cyan", () => {
    expect(tokens).toMatch(/--cyan:\s*#[0-9a-f]{6};/i);
  });

  it("draws a 2px gray, 1px cyan, 2px gray divider at every group boundary", () => {
    const boundary = ruleContaining(
      sharedStyles,
      ".ui-title-chrome-group + .ui-title-chrome-group::before",
    );

    expect(boundary).toBeDefined();
    expect(boundary).toContain("width: 5px");
    expect(boundary).toMatch(
      /var\(--line\) 0 2px,\s*var\(--cyan\) 2px 3px,\s*var\(--line\) 3px 5px/,
    );
  });

  it("shares one boundary rule across window chrome, modal chrome, and terminals", () => {
    const rule = [...sharedStyles.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(
      (match) =>
        match[1].includes(".ui-title-chrome-group + .ui-title-chrome-group::before"),
    );

    expect(rule?.[1]).toContain(".ui-title-chrome-terminals::before");
    expect(rule?.[1]).toContain(
      ".ui-window-action-group + .ui-window-action-group::before",
    );
  });

  it("stops the separated buttons from doubling the boundary", () => {
    const dropped = ruleContaining(
      sharedStyles,
      ".ui-window-action-group + .ui-window-action-group > :first-child",
    );
    const trailing = ruleContaining(
      sharedStyles,
      ".ui-window-action-group:has(+ .ui-window-action-group) > :last-child",
    );

    expect(dropped).toContain("border-left: 0");
    expect(trailing).toContain("border-right: 0");
  });

  it("keeps the modal titlebar free of its own group boundary border", () => {
    expect(modalStyles).not.toMatch(
      /\.ui-title-chrome-group\s*\+\s*\.ui-title-chrome-group[^{]*\{/,
    );
  });
});
