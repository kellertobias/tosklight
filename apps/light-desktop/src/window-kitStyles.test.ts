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

describe("desktop DataTable compatibility styles", () => {
	it("does not reintroduce a second horizontal separator", () => {
		const tableRules = ruleBodies(desktopStyles, ".ui-data-table");
		const cellRules = ruleBodies(desktopStyles, ".ui-data-table-row > span");

		expect(tableRules.length).toBeGreaterThan(0);
		expect(tableRules.join("\n")).not.toMatch(/(?:repeating-)?linear-gradient/);
		expect(tableRules.join("\n")).not.toMatch(/border-(?:top|bottom)\s*:/);
		expect(cellRules.join("\n")).not.toMatch(/border-(?:top|bottom)\s*:/);
	});
});
