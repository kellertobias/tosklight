import { build } from "esbuild";
import { chromium } from "playwright";
import { mkdtempSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { expect, it } from "vitest";
import artifactResolver from "../../../tools/artifact-paths.cjs";
const root = resolve(import.meta.dirname, "../../..");
function styles(path: string): string {
	return readFileSync(path, "utf8").replace(
		/@import "([^"]+)";/g,
		(_, name: string) => styles(resolve(dirname(path), name)),
	);
}
it("actual configurable Programmer encoder story keeps its numeric center hittable and contained", async () => {
	const evidence = mkdtempSync(
		resolve(
			artifactResolver.artifactPaths.tmp,
			"TL714-programmer-encoder-hit-",
		),
	);
	const story = JSON.stringify(
		resolve(root, "apps/ui-library/src/encoders/EncoderSurfaces.stories.tsx"),
	);
	const modals = JSON.stringify(resolve(root, "apps/ui-library/src/modals.ts"));
	const contents = `import React from'react';import{createRoot}from'react-dom/client';import meta,{ConfigurableFamily}from${story};import{ModalProvider}from${modals};createRoot(document.getElementById('app')).render(<ModalProvider>{ConfigurableFamily.render({...meta.args,...ConfigurableFamily.args})}</ModalProvider>);`;
	const bundle = await build({
		stdin: { contents, loader: "tsx", resolveDir: root },
		bundle: true,
		write: false,
		outfile: resolve(evidence, "fixture.js"),
		platform: "browser",
		jsx: "automatic",
		logLevel: "silent",
	});
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage({
			viewport: { width: 1496, height: 761 },
		});
		await page.setContent(
			`<style>${styles(resolve(root, "apps/ui-library/src/styles.css"))}body{margin:0}</style><div id="app"></div>`,
		);
		for (const file of bundle.outputFiles.filter((f) =>
			f.path.endsWith(".css"),
		))
			await page.addStyleTag({ content: file.text });
		for (const file of bundle.outputFiles.filter((f) => f.path.endsWith(".js")))
			await page.addScriptTag({ content: file.text });
		const encoder = page.getByRole("group", { name: "Enc 1 · Red" });
		const button = encoder.getByRole("button", {
			name: "Set Enc 1 · Red value",
		});
		await button.waitFor();
		await page.screenshot({ path: resolve(evidence, "encoder.png") });
		const geometry = await button.evaluate((el) => {
			const b = el.getBoundingClientRect(),
				e = el.closest(".touch-encoder")!.getBoundingClientRect();
			const hit = document.elementFromPoint(
				b.x + b.width / 2,
				b.y + b.height / 2,
			);
			return {
				button: b.toJSON(),
				encoder: e.toJSON(),
				hittable: hit === el || el.contains(hit),
			};
		});
		expect(geometry.button.height).toBeGreaterThanOrEqual(44);
		expect(geometry.button.top).toBeGreaterThanOrEqual(geometry.encoder.top);
		expect(geometry.button.bottom).toBeLessThanOrEqual(geometry.encoder.bottom);
		expect(geometry.hittable).toBe(true);
		await button.click({ timeout: 2000 });
		const dialog = page.getByRole("dialog", { name: "Enc 1 · Red value" });
		await expect.poll(() => dialog.count()).toBe(1);
		await dialog.getByRole("button", { name: "2", exact: true }).click();
		await dialog.getByRole("button", { name: "5", exact: true }).click();
		await dialog.getByRole("button", { name: "ENTER", exact: true }).click();
		await expect.poll(() => button.textContent()).toBe("25%");
	} finally {
		await browser.close();
	}
}, 20_000);
