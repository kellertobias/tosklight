import { describe, expect, it, vi } from "vitest";
import { installContextMenuPolicy } from "./contextMenuPolicy";

function contextMenu(target: EventTarget, init: MouseEventInit = {}) {
	const event = new MouseEvent("contextmenu", {
		bubbles: true,
		cancelable: true,
		...init,
	});
	target.dispatchEvent(event);
	return event;
}

describe("Architect context-menu policy", () => {
	it("never opens the browser menu, including for a macOS Ctrl-click", () => {
		const target = document.createElement("div");
		document.body.append(target);
		const uninstall = installContextMenuPolicy(document);

		expect(contextMenu(target).defaultPrevented).toBe(true);
		expect(
			contextMenu(target, { ctrlKey: true, button: 0 }).defaultPrevented,
		).toBe(true);
		uninstall();
		target.remove();
	});

	it("keeps a dedicated right-click action working", () => {
		const target = document.createElement("button");
		document.body.append(target);
		const dedicatedAction = vi.fn();
		target.addEventListener("contextmenu", (event) => {
			event.preventDefault();
			dedicatedAction();
		});
		const uninstall = installContextMenuPolicy(document);

		expect(contextMenu(target).defaultPrevented).toBe(true);
		expect(dedicatedAction).toHaveBeenCalledOnce();
		uninstall();
		target.remove();
	});

	it("stops suppressing menus after uninstall", () => {
		const target = document.createElement("div");
		document.body.append(target);
		installContextMenuPolicy(document)();

		expect(contextMenu(target).defaultPrevented).toBe(false);
		target.remove();
	});
});
