import { describe, expect, it, vi } from "vitest";
import { installDeskContextMenuPolicy } from "./deskContextMenuPolicy";

describe("desk context-menu policy", () => {
	it("suppresses an unsupported browser menu", () => {
		const target = document.createElement("input");
		document.body.append(target);
		const uninstall = installDeskContextMenuPolicy(document);
		const event = new MouseEvent("contextmenu", {
			bubbles: true,
			cancelable: true,
		});

		target.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(true);
		uninstall();
		target.remove();
	});

	it("suppresses the browser menu without swallowing a dedicated action", () => {
		const target = document.createElement("button");
		document.body.append(target);
		const dedicatedAction = vi.fn();
		target.addEventListener("contextmenu", (event) => {
			event.preventDefault();
			dedicatedAction();
		});
		const uninstall = installDeskContextMenuPolicy(document);
		const event = new MouseEvent("contextmenu", {
			bubbles: true,
			cancelable: true,
		});

		target.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(true);
		expect(dedicatedAction).toHaveBeenCalledOnce();
		uninstall();
		target.remove();
	});

	it("stops suppressing menus after uninstall", () => {
		const target = document.createElement("div");
		document.body.append(target);
		const uninstall = installDeskContextMenuPolicy(document);
		uninstall();
		const event = new MouseEvent("contextmenu", {
			bubbles: true,
			cancelable: true,
		});

		target.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(false);
		target.remove();
	});
});
