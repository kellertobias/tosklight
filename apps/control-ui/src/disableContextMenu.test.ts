import { afterEach, describe, expect, it, vi } from "vitest";
import { enableSetOnContextMenu } from "./disableContextMenu";

describe("enableSetOnContextMenu", () => {
  afterEach(() => {
    vi.useRealTimers();
    document.body.replaceChildren();
  });

  it("runs the SET action before clicking the context-menu target", () => {
    vi.useFakeTimers();
    const disable = enableSetOnContextMenu();
    const target = document.createElement("button");
    const actions: string[] = [];
    document.body.append(target);
    window.addEventListener("light:desk-action", () => actions.push("set"), { once: true });
    target.addEventListener("contextmenu", () => actions.push("legacy-context-menu"));
    target.addEventListener("click", () => actions.push("click"));
    const disabledEvent = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });

    target.dispatchEvent(disabledEvent);
    expect(disabledEvent.defaultPrevented).toBe(true);
    expect(actions).toEqual(["set"]);

    vi.runAllTimers();
    expect(actions).toEqual(["set", "click"]);
    disable();
  });

  it("restores the context menu when disabled", () => {
    const disable = enableSetOnContextMenu();
    disable();
    const enabledEvent = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    document.body.dispatchEvent(enabledEvent);
    expect(enabledEvent.defaultPrevented).toBe(false);
  });
});
