import { build } from "esbuild";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { chromium } from "playwright";
import { expect, it } from "vitest";
import artifactResolver from "../../../tools/artifact-paths.cjs";

const root = resolve(import.meta.dirname, "../../..");
function styles(path: string): string {
	return readFileSync(path, "utf8").replace(/@import "([^"]+)";/g, (_, name: string) =>
		styles(name === "@tosklight/ui/styles.css"
			? resolve(root, "apps/ui-library/src/styles.css") : resolve(dirname(path), name)));
}
it.each([1440, 768, 640])("fits Playback settings title controls at %ipx without overlap", async (width) => {
	const evidence = mkdtempSync(resolve(artifactResolver.artifactPaths.tmp, "TL714-settings-title-"));
	const component = JSON.stringify(resolve(root, "apps/light-desktop/src/components/modals/PlaybackSectionSettingsModal.tsx"));
	const bundle = await build({
		stdin: { contents: `import React from 'react';import{createRoot}from'react-dom/client';import{ModalProvider}from'@tosklight/ui/modals';import{PlaybackSectionSettingsModal}from${component};createRoot(document.getElementById('app')).render(<ModalProvider><PlaybackSectionSettingsModal surfaceId="main" onClose={()=>{}}/></ModalProvider>);`, loader: "tsx", resolveDir: root },
		bundle: true, write: false, outfile: resolve(evidence, "fixture.js"), platform: "browser", jsx: "automatic", logLevel: "silent",
		plugins: [{ name: "scoped-authority", setup(build) {
			build.onLoad({ filter: /ScreensContext\.tsx$/ }, () => ({ contents: `export const useScreens=()=>({surfaces:{playback_surfaces:{main:{name:'Main',page_source:{type:'own',page:4}}},resolved_pages:{main:4}},updatePlaybackSurface:async()=>{}});`, loader: "tsx" }));
			build.onLoad({ filter: /PlaybackSoloSettings\.tsx$/ }, () => ({ contents: `export const PlaybackSoloSettings=()=>null;`, loader: "tsx" }));
		} }],
	});
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage({ viewport: { width, height: 846 } });
		await page.setContent(`<style>${styles(resolve(root, "apps/light-desktop/src/styles.css"))}body{margin:0}</style><div id="app"></div>`);
		for (const file of bundle.outputFiles.filter((file) => file.path.endsWith(".css"))) await page.addStyleTag({ content: file.text });
		for (const file of bundle.outputFiles.filter((file) => file.path.endsWith(".js"))) await page.addScriptTag({ content: file.text });
		await page.getByRole("dialog").waitFor();
		const measured = await page.locator('.ui-modal-titlebar').evaluate((header) => {
			const title = header.querySelector<HTMLElement>('h2')!;
			const buttons = [...header.querySelectorAll<HTMLElement>('button')];
			return {
				header: header.getBoundingClientRect().toJSON(),
				title: title.getBoundingClientRect().toJSON(), titleFits: title.scrollWidth <= title.clientWidth,
				buttons: buttons.map((button) => ({ box: button.getBoundingClientRect().toJSON(), fits: button.scrollWidth <= button.clientWidth })),
			};
		});
		await page.screenshot({ path: resolve(evidence, `title-${width}.png`) });
		writeFileSync(resolve(evidence, "measurements.json"), JSON.stringify(measured, null, 2), { flag: "wx" });
		console.log(`Playback settings title evidence: ${evidence}`, JSON.stringify(measured));
		expect(measured.titleFits).toBe(true);
		for (const { box, fits } of measured.buttons) {
			expect(fits).toBe(true);
			expect(box.height).toBeGreaterThanOrEqual(44);
			expect(box.right).toBeLessThanOrEqual(measured.header.right);
			expect(box.left).toBeGreaterThanOrEqual(measured.header.left);
			if (box.top < measured.title.bottom) expect(box.left).toBeGreaterThanOrEqual(measured.title.right);
		}
		expect(measured.buttons[0].box.right).toBeLessThanOrEqual(measured.buttons[1].box.left);
		expect(measured.buttons[1].box.right).toBeLessThanOrEqual(measured.buttons[2].box.left);
		expect(await page.locator('.modal-card > [role="tablist"]').count()).toBe(0);
	} finally { await browser.close(); }
}, 20_000);
