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
import{scrollablePlaybackGeometry}from ${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/scrollableGeometry.ts")};
function App(){const viewport=useRef(null);const[maximized,setMaximized]=useState(false);window.setPaneMaximized=setMaximized;window.playbackActions=0;
return <div className="desk-grid"><PaneView pane={{id:'pane',title:'Playbacks',x:13,y:1,width:12,height:18}} maximized={maximized}><section className="playback-pane"><section className="playback-surface-controls"><div ref={viewport} className="playback-bank-viewport"><div data-playback-surface="pane:test" className="playback-fader-bank touch-layout" style={scrollablePlaybackGeometry(Array(14).fill('minmax(0, 1fr)').join(' '),20)}>{Array.from({length:280},(_,i)=><TouchPlaybackCardView key={i} model={{page:1,slot:401+i,row:Math.floor(i/20),rowUnits:1,name:'Blue Pulse',assigned:true,kind:'dynamic',className:'playback-row-compact',presentation:'buttons',hasFader:false,faderValue:0,faderLabel:'Master',faderDisplay:'OFF',actions:[{id:'go',label:'OFF',onClick:()=>window.playbackActions++}]}}/>)}</div></div></section></section></PaneView></div>}
createRoot(document.getElementById('app')).render(<App/>);`;

async function wheelToEdge(page: Page) {
	await page.locator(".playback-bank-viewport").hover();
	await page.mouse.wheel(10_000, 10_000);
	await expect.poll(() => page.evaluate(() => {
		const viewport = document.querySelector(".playback-bank-viewport")!;
		return viewport.scrollLeft >= viewport.scrollWidth - viewport.clientWidth - 1 &&
			viewport.scrollTop >= viewport.scrollHeight - viewport.clientHeight - 1;
	})).toBe(true);
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
			rows: [...new Set(cards.map(card => card.getAttribute("data-playback-row")))].length,
			count: cards.length,
			actions: (window as unknown as { playbackActions: number })
				.playbackActions,
		};
	});
}
describe("personal Playback pane touch navigation", () => {
	it("reveals column20 and position680 using Free region viewport wheel without operating Playbacks, normal/maximized/restored", async () => {
		const evidence = mkdtempSync(
			resolve(artifactResolver.artifactPaths.tmp, "TL714-pane-navigation-"),
		);
		process.stdout.write(`Personal pane browser evidence: ${evidence}\n`);
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
				viewport: { width: 1280, height: 900 },
			});
			await page.setContent(
				`<style>${styles(resolve(root, "apps/light-desktop/src/styles.css"))}body{margin:0}.desk-grid{width:1200px;height:700px}</style><div id="app"></div><p>Personal Playback pane: exact20×14, positions401–680. Free region wheel navigation reveals all cells without permanent toolbars or operating Playbacks.</p>`,
			);
			for (const file of bundle.outputFiles) {
				if (file.path.endsWith(".css"))
					await page.addStyleTag({ content: file.text });
			}
			for (const file of bundle.outputFiles) {
				if (file.path.endsWith(".js"))
					await page.addScriptTag({ content: file.text });
			}
			await page.locator(".playback-bank-viewport").waitFor();
			expect(await page.locator(".playback-solo-toolbar, .playback-bank-scroll-controls").count()).toBe(0);
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
				expect(await page.locator(".playback-bank-viewport").evaluate(element => ({ left: element.scrollLeft, top: element.scrollTop }))).toEqual({ left: 0, top: 0 });
				await page.screenshot({
					path: resolve(evidence, `initial-${index}.png`),
				});
				await wheelToEdge(page);
				const result = await inspect(page);
				expect(result.addresses).toEqual(
					Array.from({ length: 280 }, (_, i) => `1.${401 + i}`),
				);
				expect(result.finalVisible).toBe(true);
				expect(result.actions).toBe(0);
				expect(result.count).toBe(280);
				expect(result.rows).toBe(14);
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
