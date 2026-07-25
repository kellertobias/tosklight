import { describe, expect, it } from "vitest";

const sources = import.meta.glob("/src/**/*.{ts,tsx}", {
	eager: true,
	query: "?raw",
	import: "default",
}) as Record<string, string>;

function productionSources() {
	return Object.entries(sources).filter(([file]) => !file.includes(".test."));
}

describe("desktop platform boundary", () => {
	it("keeps Tauri imports inside the Tauri adapter", () => {
		const offenders = productionSources()
			.filter(([, source]) => source.includes("@tauri-apps/api"))
			.map(([file]) => file);
		expect(offenders).toEqual([
			"/src/platform/desktop/tauriDesktopBridge.ts",
		]);
	});

	it("keeps runtime detection at bridge construction", () => {
		const offenders = productionSources()
			.filter(([, source]) => source.includes("__TAURI_INTERNALS__"))
			.map(([file]) => file);
		expect(offenders).toEqual(["/src/platform/desktop/index.ts"]);
	});
});
