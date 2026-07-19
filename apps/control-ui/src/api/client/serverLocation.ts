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

export function defaultServerUrl(location = window.location): string {
	const configured = import.meta.env.VITE_LIGHT_SERVER_URL as
		| string
		| undefined;
	if (configured) return configured.replace(/\/$/, "");
	if (location.protocol === "tauri:") {
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
