import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	browserStorage,
	defaultServerUrl,
	saveServerUrl,
} from "./serverLocation";

function memoryStorage(): Storage {
	const values = new Map<string, string>();
	return {
		get length() {
			return values.size;
		},
		clear: () => values.clear(),
		getItem: (key) => values.get(key) ?? null,
		key: (index) => [...values.keys()][index] ?? null,
		removeItem: (key) => values.delete(key),
		setItem: (key, value) => values.set(key, String(value)),
	};
}

describe("desktop and browser server locations", () => {
	beforeEach(() => {
		vi.stubGlobal("localStorage", memoryStorage());
		vi.stubGlobal("sessionStorage", memoryStorage());
		vi.stubEnv("VITE_LIGHT_SERVER_URL", "");
		vi.stubGlobal("__TAURI_INTERNALS__", undefined);
	});

	afterEach(() => {
		vi.unstubAllEnvs();
		vi.unstubAllGlobals();
	});

	it.each([
		"http://tauri.localhost",
		"https://tauri.localhost",
		"tauri://localhost",
	])("packaged origin %s connects to the bundled backend", (origin) => {
		expect(defaultServerUrl(new URL(origin))).toBe("http://127.0.0.1:5000");
	});

	it("native runtime detection also supports a development webview", () => {
		vi.stubGlobal("__TAURI_INTERNALS__", {});
		expect(defaultServerUrl(new URL("http://127.0.0.1:4175"))).toBe(
			"http://127.0.0.1:5000",
		);
	});

	it.each([
		"http://127.0.0.1:4175",
		"https://desk.example",
		"http://tauri.localhost.example",
		"https://example.com/tauri.localhost",
	])("ordinary browser URL %s keeps its own origin", (url) => {
		localStorage.setItem("light.server-url", "http://saved-desk:5000");
		const location = new URL(url);
		expect(defaultServerUrl(location)).toBe(location.origin);
	});

	it("Windows respects the saved server and the temporary test override in precedence order", () => {
		const location = new URL("http://tauri.localhost");
		localStorage.setItem("light.server-url", "http://saved-desk:5000/");
		expect(defaultServerUrl(location)).toBe("http://saved-desk:5000");
		sessionStorage.setItem("light.test-server-url", "http://127.0.0.1:5100/");
		expect(defaultServerUrl(location)).toBe("http://127.0.0.1:5100");
		expect(browserStorage()).toBe(sessionStorage);
		saveServerUrl("http://edited-desk:5000/");
		expect(sessionStorage.getItem("light.server-url")).toBe(
			"http://edited-desk:5000",
		);
		expect(localStorage.getItem("light.server-url")).toBe(
			"http://saved-desk:5000/",
		);
	});

	it("the explicit build-time server override retains highest precedence", () => {
		vi.stubEnv("VITE_LIGHT_SERVER_URL", "https://configured.example/");
		sessionStorage.setItem("light.test-server-url", "http://127.0.0.1:5100");
		expect(defaultServerUrl(new URL("http://tauri.localhost"))).toBe(
			"https://configured.example",
		);
		expect(defaultServerUrl(new URL("https://browser.example"))).toBe(
			"https://configured.example",
		);
	});
});
