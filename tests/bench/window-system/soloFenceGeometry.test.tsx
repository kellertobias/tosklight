import { build } from "esbuild";
import { chromium } from "playwright";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const root = resolve(import.meta.dirname, "../../..");
const overlay = resolve(
	root,
	"apps/light-desktop/src/components/control/playbackFaderBank/SoloFenceOverlay.tsx",
);
const css = readFileSync(
	resolve(root, "apps/light-desktop/src/styles/workflows.css"),
	"utf8",
);
const source = `
import React, {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {SoloFenceOverlay} from ${JSON.stringify(overlay)};
const regions=[{id:'pair',name:'Pair',members:[{page:1,position:1},{page:1,position:3}]}];
function App(){ const [bank,setBank]=useState(null); return <div>
<h1>Solo perimeter browser proof — expanded partial joins and duplicate address</h1>
<div id="viewport" style={{height:180,width:410,overflow:'auto'}}>
<div ref={setBank} className="playback-fader-bank" style={{position:'relative',width:640,height:420,display:'grid',gap:5,gridTemplateColumns:'repeat(3,minmax(0,1fr))',gridTemplateRows:'100px 200px 110px'}}>
<article data-playback-row="0" data-playback-slot="1" style={{gridRow:'1',gridColumn:'1 / span 2'}}>1.1 Wider member</article>
<article data-playback-row="1" data-playback-slot="3" style={{gridRow:'2',gridColumn:'1'}}>1.3 Partial peer</article>
<article data-playback-row="2" data-playback-slot="1" style={{gridRow:'3',gridColumn:'1 / span 2'}}>1.1 Duplicate Wider member</article>
<SoloFenceOverlay bank={bank} page={1} regions={regions} layoutKey="initial"/>
</div></div><p>Amber perimeter joins only shared edge segments. Unequal rows and both scroll axes retain exact alignment.</p></div>; }
createRoot(document.getElementById('app')).render(<App/>);`;

describe("actual browser expanded Solo fence geometry", () => {
	it("joins partial edges at rendered gap midpoints and stays aligned after resize and two-axis scroll", async () => {
		const bundle = await build({
			stdin: {
				contents: source,
				loader: "tsx",
				resolveDir: root,
				sourcefile: "solo-fence-browser.tsx",
			},
			bundle: true,
			write: false,
			platform: "browser",
			jsx: "automatic",
			logLevel: "silent",
		});
		const browser = await chromium.launch({ headless: true });
		try {
			const page = await browser.newPage({
				viewport: { width: 1000, height: 600 },
			});
			await page.setContent(
				`<style>${css}\n:root{--amber:#ffb347}body{background:#112022;color:white;font:16px sans-serif;padding:20px}article{background:#20343f;padding:8px;box-sizing:border-box}</style><div id="app"></div>`,
			);
			await page.addScriptTag({ content: bundle.outputFiles[0].text });
			await page.waitForSelector(".playback-solo-fence-overlay line", {
				state: "attached",
				timeout: 5_000,
			});
			const inspect = () =>
				page.evaluate(() => {
					const grid = document.querySelector<HTMLElement>(
						".playback-fader-bank",
					)!;
					const bounds = grid.getBoundingClientRect();
					const cards = [...grid.querySelectorAll<HTMLElement>("article")].map(
						(card) => card.getBoundingClientRect(),
					);
					const svg = grid.querySelector<SVGSVGElement>("svg")!;
					const lines = [...svg.querySelectorAll("line")].map((line) =>
						["x1", "y1", "x2", "y2"].map((key) =>
							Number(line.getAttribute(key)),
						),
					);
					return {
						width: bounds.width,
						halfJoin: cards[1].right - bounds.left + 2.5,
						expandedRight: cards[0].right - bounds.left + 2.5,
						sharedY: cards[0].bottom - bounds.top + 2.5,
						lines,
						pointerEvents: getComputedStyle(svg).pointerEvents,
						svgWidth: svg.getBoundingClientRect().width,
						strokeWidth: getComputedStyle(svg).strokeWidth,
						svgLeft: svg.getBoundingClientRect().left,
						gridLeft: bounds.left,
						lastVisible:
							cards[2].top <
							document.querySelector("#viewport")!.getBoundingClientRect()
								.bottom,
					};
				});
			const before = await inspect();
			expect(before.pointerEvents).toBe("none");
			expect(before.svgWidth).toBe(640);
			expect(before.strokeWidth).toBe("3px");
			expect(before.lines).toContainEqual([
				before.halfJoin,
				before.sharedY,
				before.expandedRight,
				before.sharedY,
			]);
			expect(before.lines).not.toContainEqual([
				1.5,
				before.sharedY,
				before.expandedRight,
				before.sharedY,
			]);
			await page.screenshot({
				path: resolve(
					root,
					".artifacts/test/visual-inspection/release-acceptance/TL714-verification/solo-partial-browser-before-scroll.png",
				),
			});
			await page.locator("#viewport").hover();
			await page.mouse.wheel(300, 400);
			await page.waitForFunction(
				() => {
					const element = document.querySelector("#viewport")!;
					return element.scrollLeft > 0 && element.scrollTop > 0;
				},
				undefined,
				{ timeout: 5_000 },
			);
			const scrolled = await inspect();
			expect(scrolled.lines).toEqual(before.lines);
			expect(scrolled.svgLeft).toBe(scrolled.gridLeft);
			expect(scrolled.lastVisible).toBe(true);
			await page.screenshot({
				path: resolve(
					root,
					".artifacts/test/visual-inspection/release-acceptance/TL714-verification/solo-partial-browser.png",
				),
			});
			await page.evaluate(() => {
				document.querySelector<HTMLElement>(
					".playback-fader-bank",
				)!.style.width = "800px";
			});
			await page.waitForFunction(() =>
				document
					.querySelector("svg")!
					.getAttribute("viewBox")
					?.startsWith("0 0 800 "),
			);
			const resized = await inspect();
			expect(resized.lines).toContainEqual([
				resized.halfJoin,
				resized.sharedY,
				resized.expandedRight,
				resized.sharedY,
			]);
			expect(resized.expandedRight).toBeGreaterThan(before.expandedRight);
		} finally {
			await browser.close();
		}
	}, 20_000);
});
