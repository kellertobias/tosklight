import { build } from "esbuild";
import { chromium, type Page } from "playwright";
import { mkdtempSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { describe, expect, it } from "vitest";
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
const modulePath = (path: string) => JSON.stringify(resolve(root, path));
const source = `
import React,{useRef,useState}from'react';import{createRoot}from'react-dom/client';
import{PaneView}from ${modulePath("apps/ui-library/src/desktop/PaneView.tsx")};
import{TouchPlaybackCardView}from ${modulePath("apps/ui-library/src/playback/PlaybackCards.tsx")};
import{PlaybackBankScrollControls}from ${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/PlaybackBankScrollControls.tsx")};
import{scrollablePlaybackGeometry}from ${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/scrollableGeometry.ts")};
function App(){const viewport=useRef(null);const[maximized,setMaximized]=useState(false);window.setPaneMaximized=setMaximized;window.playbackActions=0;
return <div className="desk-grid"><PaneView pane={{id:'pane',title:'Playbacks',x:13,y:1,width:12,height:18}} maximized={maximized}><section className="playback-pane"><section className="playback-surface-controls"><div><div className="playback-solo-toolbar">Select Solo members · Manage Solo Regions</div><PlaybackBankScrollControls viewport={viewport}/></div><div ref={viewport} className="playback-bank-viewport"><div data-playback-surface="pane:test" className="playback-fader-bank touch-layout" style={scrollablePlaybackGeometry(Array(14).fill('minmax(0, 1fr)').join(' '),20)}>{Array.from({length:280},(_,i)=><TouchPlaybackCardView key={i} model={{page:1,slot:401+i,row:Math.floor(i/20),rowUnits:1,name:'Blue Pulse',assigned:true,kind:'dynamic',className:'playback-row-compact',presentation:'buttons',hasFader:false,faderValue:0,faderLabel:'Master',faderDisplay:'OFF',actions:[{id:'go',label:'OFF',onClick:()=>window.playbackActions++}]}}/>)}</div></div></section></section></PaneView></div>}
createRoot(document.getElementById('app')).render(<App/>);`;

async function clickToEdge(page: Page, direction: "right" | "down") {
	const button = page.getByRole("button", {
		name: `Scroll Playbacks ${direction}`,
	});
	for (let i = 0; i < 20 && (await button.isEnabled()); i++) {
		await button.click();
		await page.waitForTimeout(20);
	}
	expect(await button.isDisabled()).toBe(true);
}
async function inspect(page: Page) {
	return page.evaluate(() => {
		const viewport = document.querySelector(".playback-bank-viewport")!;
		const bounds = viewport.getBoundingClientRect();
		const cards = [...document.querySelectorAll(".touch-playback-card")];
		const last = cards.at(-1)!.getBoundingClientRect();
		return {
			width: document.querySelector(".desk-pane")!.getBoundingClientRect()
				.width,
			addresses: [
				...document.querySelectorAll(".single-button-playback-identity strong"),
			].map((el) => el.textContent),
			finalVisible:
				last.left >= bounds.left &&
				last.right <= bounds.right &&
				last.top >= bounds.top &&
				last.bottom <= bounds.bottom,
			controls: [
				...document.querySelectorAll(".playback-bank-scroll-controls button"),
			].map((el) => {
				const r = el.getBoundingClientRect();
				return { width: r.width, height: r.height };
			}),
			actions: (window as unknown as { playbackActions: number })
				.playbackActions,
		};
	});
}
describe("personal Playback pane touch navigation", () => {
	it("reveals column20 and position680 using visible arrows without operating Playbacks, normal/maximized/restored", async () => {
		const evidence = mkdtempSync(
			resolve(artifactResolver.artifactPaths.tmp, "TL714-pane-navigation-"),
		);
		process.stdout.write(`Personal pane browser evidence: ${evidence}\n`);
		const bundle = await build({
			stdin: { contents: source, loader: "tsx", resolveDir: root },
			bundle: true,
			write: false,
			platform: "browser",
			jsx: "automatic",
			logLevel: "silent",
		});
		const browser = await chromium.launch({ headless: true });
		try {
			const page = await browser.newPage({
				viewport: { width: 1280, height: 900 },
			});
			await page.setContent(
				`<style>${styles(resolve(root, "apps/light-desktop/src/styles.css"))}body{margin:0}.desk-grid{width:1200px;height:700px}</style><div id="app"></div><p>Personal Playback pane: exact20×14, positions401–680. Four visible44px arrows reveal all cells without operating Playbacks.</p>`,
			);
			await page.addScriptTag({ content: bundle.outputFiles[0].text });
			await page
				.getByRole("navigation", { name: "Scroll Playbacks" })
				.waitFor();
			const widths = [];
			for (const [index, maximized] of [false, true, false].entries()) {
				await page.evaluate(
					(value) =>
						(
							window as unknown as { setPaneMaximized(value: boolean): void }
						).setPaneMaximized(value),
					maximized,
				);
				await page.waitForTimeout(30);
				await page.evaluate(() =>
					document.querySelector(".playback-bank-viewport")!.scrollTo(0, 0),
				);
				await page.waitForTimeout(30);
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
				await page.screenshot({
					path: resolve(evidence, `initial-${index}.png`),
				});
				await clickToEdge(page, "right");
				await clickToEdge(page, "down");
				const result = await inspect(page);
				expect(result.addresses).toEqual(
					Array.from({ length: 280 }, (_, i) => `1.${401 + i}`),
				);
				expect(result.finalVisible).toBe(true);
				expect(result.actions).toBe(0);
				for (const control of result.controls) {
					expect(control.width).toBeGreaterThanOrEqual(44);
					expect(control.height).toBeGreaterThanOrEqual(44);
				}
				widths.push(result.width);
				await page.screenshot({
					path: resolve(evidence, `edge680-${index}.png`),
				});
			}
			expect(widths[0]).toBeLessThan(610);
			expect(widths[1]).toBe(1200);
			expect(widths[2]).toBe(widths[0]);
		} finally {
			await browser.close();
		}
	}, 15000);
});
