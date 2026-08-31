/// <reference types="vite/client" />

import { describe, expect, it } from "vitest";
import desktopStyles from "./window-kit.css?raw";

function ruleBodies(source: string, selector: string): string[] {
	const escaped = selector
		.trim()
		.split(/\s+/)
		.map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
		.join("\\s*");
	return [...source.matchAll(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, "g"))].map(
		(match) => match[1],
	);
}

describe("desktop DataTable styles", () => {
	it("leaves generic table separators entirely package-owned", () => {
		const tableRules = ruleBodies(desktopStyles, ".ui-data-table");
		const cellRules = ruleBodies(desktopStyles, ".ui-data-table-row > span");

		expect(tableRules).toHaveLength(0);
		expect(cellRules).toHaveLength(0);
	});
});

describe("Patch title-action styles", () => {
	it("leaves the divider between adjacent action groups package-owned", () => {
		const patchRules = ruleBodies(
			desktopStyles,
			".show-patch-layout > .ui-window-header .ui-window-action-group + .ui-window-action-group",
		);
		const groupRules = ruleBodies(
			desktopStyles,
			".ui-window-action-group + .ui-window-action-group",
		);

		expect(patchRules).toHaveLength(0);
		expect(groupRules).toHaveLength(0);
	});
});
