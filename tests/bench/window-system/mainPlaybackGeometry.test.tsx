import { build } from "esbuild";
import { chromium } from "playwright";
import { mkdtempSync, readFileSync } from "node:fs";
import artifactResolver from "../../../tools/artifact-paths.cjs";
import { dirname, resolve } from "node:path";
import { describe, expect, it } from "vitest";

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
const css = styles(resolve(root, "apps/light-desktop/src/styles.css"));
const modulePath = (path: string) => JSON.stringify(resolve(root, path));
const source = `
import React,{useRef} from 'react';import{createRoot}from'react-dom/client';
import {CommandSection} from ${modulePath("apps/ui-library/src/command/CommandSection.tsx")};
import {TouchPlaybackCardView} from ${modulePath("apps/ui-library/src/playback/PlaybackCards.tsx")};
import {PlaybackBankScrollControls} from ${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/PlaybackBankScrollControls.tsx")};
import {scrollablePlaybackGeometry} from ${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/scrollableGeometry.ts")};
function Bank(){const viewport=useRef(null);
return <section className="playback-surface-controls"><div className="playback-main-toolbar"><div className="playback-solo-toolbar"><button className="ui-button">Select Solo members</button><button className="ui-button">Manage Solo Regions</button></div>
<PlaybackBankScrollControls viewport={viewport}/></div>
<div ref={viewport} className="playback-bank-viewport"><div data-playback-surface="main" className="playback-fader-bank touch-layout" style={scrollablePlaybackGeometry('minmax(0, 4fr) minmax(0, 1fr)',15,true)}>
{Array.from({length:30},(_,i)=><TouchPlaybackCardView key={i} model={{page:1,slot:i<15?i+1:i+86,row:i<15?0:1,rowUnits:i<15?4:1,name:'Beam Audience',assigned:true,kind:'group-master',className:i<15?'':'playback-row-compact',hasFader:i<15,presentation:i<15?'fader':'buttons',faderValue:100,faderLabel:'Master',faderDisplay:'100%',summary:{label:'28 fixtures'},actions:i<15?(i===0?[{id:'select',label:'SELECT'},{id:'flash',label:'FLASH'},{id:'select-fixtures',label:'SELECT FIXTURES'}]:[{id:'select',label:'SELECT'},{id:'flash',label:'FLASH'}]):[{id:'go',label:'GO +'}]}}/>)}</div></div></section>}
createRoot(document.getElementById('app')).render(<div className="app-shell"><aside className="left-dock">ToskLight</aside><main className="workspace">Main15 faders1–15 and buttons101–115 · literal1440×{window.innerHeight} software geometry</main><CommandSection mode="playbacks" hardware={false} commandLine={<div className="command-line-bar">Command line</div>} programmer={null} playbacks={<Bank/>} programmerTools={null} playbackTools={<div>Page1 · Cue Fade · Release · Speed groups</div>} hardwareTools={null}/></div>);`;

describe("Main literal fifteen-column software geometry", () => {
	it.each([
		900, 846,
	])("initially exposes full fader identity/actions at1440×%i, with touch navigation", async (height) => {
		const evidence = mkdtempSync(
			resolve(artifactResolver.artifactPaths.tmp, "TL714-main15-"),
		);
		process.stdout.write("Main15 browser evidence: " + evidence + "\n");
		const bundle = await build({
			stdin: { contents: source, loader: "tsx", resolveDir: root },
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
				viewport: { width: 1440, height },
			});
			await page.setContent(
				`<style>${css}body{margin:0}#app{height:100vh}</style><div id="app"></div>`,
			);

			for (const file of bundle.outputFiles) {
				if (file.path.endsWith(".css"))
					await page.addStyleTag({ content: file.text });
			}
			for (const file of bundle.outputFiles) {
				if (file.path.endsWith(".js"))
					await page.addScriptTag({ content: file.text });
			}
			await page.waitForSelector(".touch-playback-card");
			await page.screenshot({
				path: resolve(evidence, `initial-${height}.png`),
			});
			const measured = await page.evaluate(() => {
				const rect = (s: string) =>
					document.querySelector(s)!.getBoundingClientRect().toJSON();
				const actions = [
					...document.querySelectorAll(".vertical-touch-fader-actions button"),
				].map((e) => e.getBoundingClientRect().toJSON());
				const slider = rect(".vertical-touch-fader");
				return {
					card: rect(".touch-playback-card"),
					viewport: rect(".playback-bank-viewport"),
					identity: rect(".playback-identity"),
					workspace: rect(".workspace"),
					slider,
					actions,
					addresses: [
						...document.querySelectorAll(
							".single-button-playback-identity strong",
						),
					].map((e) => e.textContent),
					labels: [
						...document.querySelectorAll(
							".vertical-touch-fader-actions button",
						),
					].map((e) => ({
						text: e.textContent,
						scroll: e.scrollWidth,
						client: e.clientWidth,
					})),
					columns: getComputedStyle(
						document.querySelector(".playback-fader-bank")!,
					).gridTemplateColumns.split(" ").length,
				};
			});
			expect(measured.slider.bottom).toBeLessThanOrEqual(
				measured.viewport.bottom,
			);
			expect(measured.identity.top).toBeGreaterThanOrEqual(
				measured.viewport.top,
			);
			for (const action of measured.actions)
				expect(action.bottom).toBeLessThanOrEqual(measured.viewport.bottom);
			expect(measured.workspace.height).toBeGreaterThanOrEqual(350);
			expect(measured.columns).toBe(15);
			expect(measured.card.width).toBeGreaterThanOrEqual(112);
			expect(measured.slider.height).toBeGreaterThanOrEqual(120);
			for (const action of measured.actions) {
				expect(action.width).toBeGreaterThanOrEqual(44);
				expect(action.height).toBeGreaterThanOrEqual(44);
			}
			expect(measured.actions[0].right).toBeLessThanOrEqual(
				measured.actions[1].left,
			);
			expect(measured.addresses).toEqual(
				Array.from({ length: 15 }, (_, i) => `1.${101 + i}`),
			);
			for (const label of measured.labels)
				expect(label.scroll).toBeLessThanOrEqual(label.client);
			expect(
				await page
					.getByRole("button", { name: "Scroll Playbacks left" })
					.isDisabled(),
			).toBe(true);
			expect(
				await page
					.getByRole("button", { name: "Scroll Playbacks up" })
					.isDisabled(),
			).toBe(true);
			await page
				.getByRole("button", { name: "Scroll Playbacks right" })
				.click();
			expect(
				await page
					.getByRole("button", { name: "Scroll Playbacks down" })
					.isDisabled(),
			).toBe(true);
			// The literal two rows fit initially. Also exercise the same directional controls when a constrained viewport needs vertical scrolling.
			await page.evaluate(() => {
				const viewport = document.querySelector<HTMLElement>(
					".playback-bank-viewport",
				)!;
				viewport.style.flex = "none";
				viewport.style.height = "160px";
			});
			await page
				.getByRole("button", { name: "Scroll Playbacks down" })
				.click({ timeout: 3000 });

			await page.waitForFunction(
				() => {
					const v = document.querySelector(".playback-bank-viewport")!;
					return v.scrollTop > 0 && v.scrollLeft > 0;
				},
				undefined,
				{ timeout: 3000 },
			);
			await page.screenshot({
				path: resolve(evidence, "start.png"),
			});
			const gap = await page.evaluate(() => {
				const viewport = document
					.querySelector(".playback-bank-viewport")!
					.getBoundingClientRect();
				const card = [...document.querySelectorAll("article")]
					.map((e) => e.getBoundingClientRect())
					.find(
						(r) =>
							r.right > viewport.left + 10 && r.right < viewport.right - 10,
					)!;
				return { x: card.right + 1, y: viewport.top + 10 };
			});
			await page.mouse.move(gap.x, gap.y);
			await page.mouse.wheel(0, 4000);
			await page.mouse.wheel(4000, 0);

			await page.waitForFunction(
				() => {
					const v = document.querySelector(".playback-bank-viewport")!;
					return (
						v.scrollTop + v.clientHeight >= v.scrollHeight - 1 &&
						v.scrollLeft + v.clientWidth >= v.scrollWidth - 1
					);
				},
				undefined,
				{ timeout: 3000 },
			);
			expect(
				await page
					.getByRole("button", { name: "Scroll Playbacks right" })
					.isDisabled(),
			).toBe(true);
			expect(
				await page
					.getByRole("button", { name: "Scroll Playbacks down" })
					.isDisabled(),
			).toBe(true);
			const visible = await page.evaluate(() => {
				const v = document
					.querySelector(".playback-bank-viewport")!
					.getBoundingClientRect();
				const last = [...document.querySelectorAll("article")]
					.at(-1)!
					.getBoundingClientRect();
				return (
					last.left >= v.left &&
					last.right <= v.right &&
					last.top >= v.top &&
					last.bottom <= v.bottom
				);
			});
			expect(visible).toBe(true);
			await page.screenshot({
				path: resolve(evidence, "scrolled.png"),
			});
		} finally {
			await browser.close();
		}
	}, 15000);
});
