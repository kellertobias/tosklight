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
		(_, name: string) =>
			styles(
				name === "@tosklight/ui/styles.css"
					? resolve(root, "apps/ui-library/src/styles.css")
					: resolve(dirname(path), name),
			),
	);
}
it("keeps real ASSIGN interceptor text within narrow tall and short slots and opens exact configuration", async () => {
	const evidence = mkdtempSync(
		resolve(artifactResolver.artifactPaths.tmp, "TL714-playback-interceptor-"),
	);
	const modulePath = (path: string) => JSON.stringify(resolve(root, path));
	const contents = `import React from'react';import{createRoot}from'react-dom/client';
import{PlaybackConfigurationTarget}from${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/SlotControls.tsx")};
import{createSlotInterceptors}from${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/slotActions.ts")};
const controller={state:{playbackSetArmed:true},activePageNumber:4,assignmentPending:false,hardware:false,slots:[],topology:{cueLists:[]},dispatch:()=>{},setConfiguration:c=>window.configured=c};
createRoot(document.getElementById('app')).render(<section className="playback-fader-bank" style={{display:'flex',gap:10}}>{[1,101].map((slot,i)=>{const handlers=createSlotInterceptors(controller,null,null,slot,null,1);return <article key={slot} style={{position:'relative',width:69,height:i?55:204}} onClickCapture={handlers.interceptClick} onPointerDownCapture={handlers.interceptPointer}><PlaybackConfigurationTarget controller={controller} playback={null} slot={slot}/></article>})}</section>);`;
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
			viewport: { width: 500, height: 300 },
		});
		await page.setContent(
			`<style>${styles(resolve(root, "apps/light-desktop/src/styles.css"))}body{margin:0}</style><div id="app"></div>`,
		);
		for (const file of bundle.outputFiles.filter((f) =>
			f.path.endsWith(".css"),
		))
			await page.addStyleTag({ content: file.text });
		for (const file of bundle.outputFiles.filter((f) => f.path.endsWith(".js")))
			await page.addScriptTag({ content: file.text });
		await page.locator(".playback-configuration-target").first().waitFor();
		await page.screenshot({ path: resolve(evidence, "interceptors.png") });
		const boxes = await page
			.locator(".playback-configuration-target")
			.evaluateAll((els) =>
				els.map((el) => {
					const b = el.getBoundingClientRect();
					return {
						box: b.toJSON(),
						children: [...el.children].map((c) => ({
							box: c.getBoundingClientRect().toJSON(),
							width: c.clientWidth,
							scrollWidth: c.scrollWidth,
						})),
						height: el.clientHeight,
						scrollHeight: el.scrollHeight,
					};
				}),
			);
		for (const item of boxes) {
			expect(item.box.width).toBeGreaterThanOrEqual(44);
			expect(item.box.height).toBeGreaterThanOrEqual(44);
			expect(item.scrollHeight).toBeLessThanOrEqual(item.height);
			for (const child of item.children) {
				expect(child.box.left).toBeGreaterThanOrEqual(item.box.left);
				expect(child.box.right).toBeLessThanOrEqual(item.box.right);
				expect(child.box.top).toBeGreaterThanOrEqual(item.box.top);
				expect(child.box.bottom).toBeLessThanOrEqual(item.box.bottom);
				expect(child.scrollWidth).toBeLessThanOrEqual(child.width);
			}
		}
		await page.locator("article").nth(1).click();
		await expect
			.poll(() =>
				page.evaluate(
					() =>
						(
							window as unknown as {
								configured: { page: number; slot: number };
							}
						).configured,
				),
			)
			.toMatchObject({ page: 4, slot: 101 });
	} finally {
		await browser.close();
	}
}, 20_000);
