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
const source = `
import React,{useState}from'react';import{createRoot}from'react-dom/client';
import{TouchPlaybackCardView}from ${JSON.stringify(resolve(root, "apps/ui-library/src/playback/PlaybackCards.tsx"))};
import{SoloFenceOverlay}from ${JSON.stringify(resolve(root, "apps/light-desktop/src/components/control/playbackFaderBank/SoloFenceOverlay.tsx"))};
const regions=[{id:'row',name:'Beam Show Intensity',members:[401,402,403,481].map(position=>({page:1,position}))},{id:'square',name:'Square',members:[421,422,441,442].map(position=>({page:1,position}))},...[461,462].map(position=>({id:String(position),name:'Separate',members:[{page:1,position}]}))];
const cells=[[401,0,0],[402,0,1],[403,0,2],[421,2,0],[422,2,1],[441,3,0],[442,3,1],[461,4,0],[462,4,1],[481,5,2]];
function App(){const[bank,setBank]=useState(null);const[page,setPage]=useState(1);window.setPage=setPage;window.actions=0;
return <div className="playback-bank-viewport" style={{width:420,height:300,overflow:'auto'}}><div ref={setBank} className="playback-fader-bank touch-layout" style={{width:639.7,height:540,gridTemplateColumns:'repeat(3,minmax(0,1fr))',gridTemplateRows:'repeat(6,minmax(0,1fr))'}}>
{cells.map(([position,row,column])=><TouchPlaybackCardView key={position} model={{page,slot:position,row,rowUnits:1,name:'Beam Show Intensity',assigned:true,kind:'cue_list',className:'playback-row-compact playback-solo-member',presentation:'buttons',hasFader:false,faderValue:0,faderLabel:'Master',faderDisplay:'OFF',style:{gridRow:row+1,gridColumn:column+1},actions:[{id:'go',label:'GO',onClick:()=>window.actions++}]}}/>)}
<SoloFenceOverlay bank={bank} page={page} regions={regions} layoutKey={page}/></div></div>}
createRoot(document.getElementById('app')).render(<App/>);`;
async function geometry(page: Page) {
	return page.evaluate(() => {
		const bank = document.querySelector<HTMLElement>(".playback-fader-bank")!;
		const bounds = bank.getBoundingClientRect();
		const cells = [...bank.querySelectorAll<HTMLElement>("article")].map(
			(card) => {
				const rect = card.getBoundingClientRect();
				return {
					position: Number(card.dataset.playbackSlot),
					left: rect.left - bounds.left,
					right: rect.right - bounds.left,
					top: rect.top - bounds.top,
					bottom: rect.bottom - bounds.top,
					border: getComputedStyle(card).borderTopColor,
				};
			},
		);
		return {
			paths: [...bank.querySelectorAll("svg path")].map((path) =>
				path.getAttribute("d"),
			),
			strokeWidth: getComputedStyle(bank.querySelector("svg")!).strokeWidth,
			halfGap: parseFloat(getComputedStyle(bank).gap) / 2,
			cells,
			pathBounds: [...bank.querySelectorAll<SVGPathElement>("svg path")].map(
				(path) => {
					const box = path.getBBox();
					return {
						left: box.x,
						right: box.x + box.width,
						top: box.y,
						bottom: box.y + box.height,
					};
				},
			),
			lines: [...bank.querySelectorAll<SVGPathElement>("svg path")].flatMap(
				(path) => {
					const total = path.getTotalLength();
					return Array.from({ length: Math.ceil(total) }, (_, index) => {
						const a = path.getPointAtLength(index),
							b = path.getPointAtLength(Math.min(index + 1, total));
						return [a.x, a.y, b.x, b.y];
					});
				},
			),
			stroke: getComputedStyle(bank.querySelector("svg")!).stroke,
			actions: (window as unknown as { actions: number }).actions,
		};
	});
}
function assertOuterUnion(value: Awaited<ReturnType<typeof geometry>>) {
	const cell = (position: number) =>
		value.cells.find((item) => item.position === position)!;
	const overlaps = (a: number, b: number, start: number, end: number) =>
		Math.min(b, end) - Math.max(a, start) > 0.02;
	for (const [left, right] of [
		[401, 402],
		[402, 403],
		[421, 422],
		[441, 442],
	]) {
		const a = cell(left),
			b = cell(right),
			seam = (a.right + b.left) / 2;
		expect(
			value.lines.filter(
				([x1, y1, x2, y2]) =>
					Math.abs(x1 - seam) < 0.02 &&
					Math.abs(x2 - seam) < 0.02 &&
					overlaps(y1, y2, a.top, a.bottom),
			),
		).toEqual([]);
	}
	for (const [top, bottom] of [
		[421, 441],
		[422, 442],
	]) {
		const a = cell(top),
			b = cell(bottom),
			seam = (a.bottom + b.top) / 2;
		expect(
			value.lines.filter(
				([x1, y1, x2, y2]) =>
					Math.abs(y1 - seam) < 0.02 &&
					Math.abs(y2 - seam) < 0.02 &&
					overlaps(x1, x2, a.left, a.right),
			),
		).toEqual([]);
	}
	const island = cell(481);
	expect(
		value.pathBounds.filter(
			(box) =>
				box.left >= island.left - value.halfGap - 0.02 &&
				box.right <= island.right + value.halfGap + 0.02 &&
				box.top >= island.top - value.halfGap - 0.02 &&
				box.bottom <= island.bottom + value.halfGap + 0.02,
		),
	).toHaveLength(1);
	expect(value.cells.every((card) => card.border !== value.stroke)).toBe(true);
	for (const position of [461, 462]) {
		const own = cell(position);
		expect(
			value.pathBounds.filter(
				(box) =>
					box.left >= own.left - value.halfGap - 0.02 &&
					box.right <= own.right + value.halfGap + 0.02 &&
					box.top >= own.top - value.halfGap - 0.02 &&
					box.bottom <= own.bottom + value.halfGap + 0.02,
			),
		).toHaveLength(1);
	}
	expect(value.actions).toBe(0);
}
describe("production Playback cards Solo union paint", () => {
	it("outlines contiguous 401–403 and a 2×2 union without amber internal seams, preserving a disconnected island and page isolation", async () => {
		const evidence = mkdtempSync(
			resolve(artifactResolver.artifactPaths.tmp, "TL714-solo-union-"),
		);
		process.stdout.write(`Solo union browser evidence: ${evidence}\n`);
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
				viewport: { width: 1000, height: 700 },
			});
			await page.setContent(
				`<style>${styles(resolve(root, "apps/light-desktop/src/styles.css"))}body{padding:24px;background:#111}</style><div id="app"></div>`,
			);
			for (const file of bundle.outputFiles)
				if (file.path.endsWith(".css"))
					await page.addStyleTag({ content: file.text });
			for (const file of bundle.outputFiles)
				if (file.path.endsWith(".js"))
					await page.addScriptTag({ content: file.text });
			await page
				.locator(".playback-solo-fence-overlay path")
				.first()
				.waitFor({ state: "attached" });
			const before = await geometry(page);
			assertOuterUnion(before);
			expect(before.paths).toHaveLength(5);
			expect(before.paths.every((path) => path?.includes(" Q "))).toBe(true);
			expect(before.strokeWidth).toBe("2px");
			await page.locator(".playback-bank-viewport").evaluate((element) => {
				(element as HTMLElement).style.width = "850px";
				(element as HTMLElement).style.height = "560px";
			});
			await page.screenshot({ path: resolve(evidence, "union-before.png") });
			await page.locator(".playback-bank-viewport").evaluate((element) => {
				(element as HTMLElement).style.width = "420px";
				(element as HTMLElement).style.height = "300px";
			});
			await page.locator(".playback-bank-viewport").hover();
			await page.mouse.wheel(300, 400);
			await expect
				.poll(() =>
					page
						.locator(".playback-bank-viewport")
						.evaluate((el) => el.scrollTop),
				)
				.toBeGreaterThan(0);
			expect((await geometry(page)).lines).toEqual(before.lines);
			await page.evaluate(
				() =>
					(document.querySelector<HTMLElement>(
						".playback-fader-bank",
					)!.style.width = "800.35px"),
			);
			await expect
				.poll(() => page.locator("svg").getAttribute("viewBox"))
				.toMatch(/^0 0 800/);
			assertOuterUnion(await geometry(page));
			await page.screenshot({
				path: resolve(evidence, "union-scrolled-resized.png"),
			});
			await page.evaluate(() =>
				(window as unknown as { setPage(page: number): void }).setPage(2),
			);
			await expect.poll(() => page.locator("svg path").count()).toBe(0);
		} finally {
			await browser.close();
		}
	}, 20000);
});
