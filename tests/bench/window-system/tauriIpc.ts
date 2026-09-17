import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import type { Page } from "@playwright/test";

/**
 * Answers a Tauri webview's IPC through Tauri's public `@tauri-apps/api/mocks` module, installed
 * before the application's own modules run. Use it for a sibling desktop app served by Vite in a
 * plain browser page, where no native host exists.
 */
export type TauriIpcHandler<Context> = (
	command: string,
	args: Record<string, unknown> | undefined,
	context: Context,
) => unknown;

const MOCKS_PATH = "/__bench/tauri-api-mocks.js";

function tauriMocksSource(): string {
	const require = createRequire(import.meta.url);
	const commonJs = require.resolve("@tauri-apps/api/mocks");
	return readFileSync(path.join(path.dirname(commonJs), "mocks.js"), "utf8");
}

/**
 * Installs `handler` as the page's IPC answer for every document it loads. The handler runs in the
 * page, so it must be self-contained and may only use the serializable `context`.
 */
export async function mockTauriIpc<Context>(
	page: Page,
	handler: TauriIpcHandler<Context>,
	context: Context,
): Promise<void> {
	const mocks = tauriMocksSource();
	const installer = `<script type="module">
import { mockIPC } from ${JSON.stringify(MOCKS_PATH)};
const context = ${JSON.stringify(context)};
const handler = (${handler.toString()});
mockIPC((command, args) => handler(command, args, context));
</script>`;
	await page.route(
		(url) => url.pathname === MOCKS_PATH,
		(route) =>
			route.fulfill({ contentType: "text/javascript", body: mocks }),
	);
	await page.route("**/*", async (route) => {
		if (route.request().resourceType() !== "document") return route.fallback();
		const response = await route.fetch();
		const html = await response.text();
		if (!html.includes("<head>"))
			throw new Error("The page has no <head> to install the Tauri IPC mock in");
		await route.fulfill({
			response,
			body: html.replace("<head>", `<head>${installer}`),
		});
	});
}
