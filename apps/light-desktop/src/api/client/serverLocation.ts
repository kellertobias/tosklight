function persistentBrowserStorage(): Storage | null {
	const storage = globalThis.localStorage;
	return storage && typeof storage.getItem === "function" ? storage : null;
}

function browserSessionStorage(): Storage | null {
	const storage = globalThis.sessionStorage;
	return storage && typeof storage.getItem === "function" ? storage : null;
}

export function browserStorage(): Storage | null {
	const session = browserSessionStorage();
	return session?.getItem("light.test-server-url")
		? session
		: persistentBrowserStorage();
}

export function defaultServerUrl(
	location: Pick<Location, "protocol" | "hostname" | "origin"> = window.location,
): string {
	const configured = import.meta.env.VITE_LIGHT_SERVER_URL as
		| string
		| undefined;
	if (configured) return configured.replace(/\/$/, "");
	const nativeWindow = Boolean(
		(globalThis as typeof globalThis & { __TAURI_INTERNALS__?: unknown })
			.__TAURI_INTERNALS__,
	);
	// Windows serves packaged assets from tauri.localhost; that origin is not the API server.
	const nativeOrigin =
		location.protocol === "tauri:" ||
		((location.protocol === "http:" || location.protocol === "https:") &&
			location.hostname === "tauri.localhost");
	if (nativeWindow || nativeOrigin) {
		return (
			browserSessionStorage()?.getItem("light.test-server-url") ||
			persistentBrowserStorage()?.getItem("light.server-url") ||
			"http://127.0.0.1:5000"
		).replace(/\/$/, "");
	}
	return location.origin;
}

export function configuredServerUrl(): string {
	return defaultServerUrl();
}

export function saveServerUrl(value: string): void {
	const url = new URL(value.trim());
	if (url.protocol !== "http:" && url.protocol !== "https:") {
		throw new Error("Server URL must use http or https");
	}
	browserStorage()?.setItem(
		"light.server-url",
		url.toString().replace(/\/$/, ""),
	);
}
