import { build } from "esbuild";
import { chromium } from "playwright";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import artifactResolver from "../../../tools/artifact-paths.cjs";
import { expect, it } from "vitest";

const root = resolve(import.meta.dirname, "../../..");
function styles(path: string): string {
	return readFileSync(path, "utf8").replace(/@import "([^"]+)";/g,
		(_, name: string) => styles(name === "@tosklight/ui/styles.css"
			? resolve(root, "apps/ui-library/src/styles.css") : resolve(dirname(path), name)));
}
const modulePath = (path: string) => JSON.stringify(resolve(root, path));
const fixture = (main: boolean, hardware: boolean) => `
import React from 'react';import {createRoot}from'react-dom/client';
import {CommandSection} from ${modulePath("apps/ui-library/src/command/CommandSection.tsx")};
import {ScreenPlaybackFit} from ${modulePath("apps/light-desktop/src/features/screens/ScreenPlaybackFit.tsx")};
import {TouchPlaybackCardView,HardwarePlaybackCardView} from ${modulePath("apps/ui-library/src/playback/PlaybackCards.tsx")};
const Card=${hardware ? "HardwarePlaybackCardView" : "TouchPlaybackCardView"};
const main=${main};const hardware=${hardware};
function Bank(){const [actionCount,setActionCount]=React.useState(3);window.setActionCount=setActionCount;return <ScreenPlaybackFit><section className={main?'playback-surface-controls':'screen-playbacks'}>
<div className="playback-bank-fixed"><div data-playback-surface={main?'main':'screen:test'} className={'playback-fader-bank '+(hardware?'hardware-layout':'touch-layout')}
style={{gridTemplateColumns:'repeat('+(main?15:10)+',minmax(0,1fr))',gridTemplateRows:main?(hardware?'2fr 1fr':'4fr 1fr'):(hardware?'2fr repeat(5,1fr)':'4fr repeat(5,1fr)')}}>
{Array.from({length:main?30:55},(_,i)=>{const fader=i<(main?15:5);const row=main?(fader?0:1):(fader?0:1+Math.floor((i-5)/10));const col=main?i%15:(fader?i*2:(i-5)%10);
return <Card key={i} model={{page:1,slot:fader?(main?i+1:301+i):(main?101+i-15:201+i-5),row,rowUnits:fader?(hardware?2:4):1,name:'Beam Stage',assigned:true,kind:'group-master',className:fader?'':'playback-row-compact',hasFader:fader,presentation:fader?(main?'fader':'push_encoder'):'buttons',faderValue:100,faderLabel:'Master',faderDisplay:'100%',summary:{label:'28 Fixtures'},style:{gridRow:row+1,gridColumn:(col+1)+(fader&&!main?' / span 2':'')},actions:fader&&main?[{id:'select',label:'SELECT'},{id:'flash',label:'FLASH'},{id:'off',label:'OFF'}].slice(0,actionCount):[{id:'go',label:'GO'}]}}/>})}
</div></div>{!main&&<aside className="screen-page-controls"><button onClick={()=>window.pageChanged=true}>PAGE UP</button></aside>}</section></ScreenPlaybackFit>}
createRoot(document.getElementById('app')).render(main?<div className="app-shell"><aside className="left-dock">Desk</aside><main className="workspace"/><CommandSection className="screen-bank-fit" mode="playbacks" hardware={hardware} commandLine={<div className="command-line-bar">Command</div>} programmer={null} playbacks={<Bank/>} programmerTools={null} playbackTools={<div>Page and Fade tools</div>} hardwareTools={<div>Hardware</div>}/></div>:<div className="screen-shell utility-content"><div className="native-drag-strip"><button>X</button></div><Bank/></div>);
`;

async function openFixture(main: boolean, hardware: boolean, height: number) {
	const evidence = mkdtempSync(resolve(artifactResolver.artifactPaths.tmp, "TL714-screen-fit-"));
	process.stdout.write(`Screen fit browser evidence: ${evidence}\n`);
	const bundle = await build({
		stdin: { contents: fixture(main, hardware), loader: "tsx", resolveDir: root },
		bundle: true, write: false, outfile: resolve(evidence, "fixture.js"),
		platform: "browser", jsx: "automatic", logLevel: "silent",
	});
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: main ? 1440 : 1200, height } });
	await page.setContent(`<style>${styles(resolve(root, "apps/light-desktop/src/styles.css"))}body{margin:0}#app{height:100vh}</style><div id="app"></div>`);
	for (const file of bundle.outputFiles.filter((file) => file.path.endsWith(".css")))
		await page.addStyleTag({ content: file.text });
	for (const file of bundle.outputFiles.filter((file) => file.path.endsWith(".js")))
		await page.addScriptTag({ content: file.text });
	await page.waitForSelector("article.playback-card");
	await page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))));
	return { browser, page, evidence };
}

it.each([
	[true, false, 846], [true, true, 846], [true, false, 900],
	[false, false, 800], [false, true, 800],
])("fits every authored cell on entry: Main=%s hardware=%s height=%i", async (main, hardware, height) => {
	const { browser, page, evidence } = await openFixture(Boolean(main), Boolean(hardware), Number(height));
	try {
		await page.screenshot({ path: resolve(evidence, "configured-fit.png") });
		const measured = await page.evaluate(() => {
			const bank = document.querySelector<HTMLElement>(".playback-fader-bank")!;
			const bounds = bank.getBoundingClientRect();
			const boxes = (selector: string) => [...bank.querySelectorAll(selector)].map((el) => el.getBoundingClientRect().toJSON());
			return {
				bounds: bounds.toJSON(), cards: boxes("article"),
				actions: boxes(".vertical-touch-fader-actions button,.single-button-playback-action,.touch-encoder-push,.touch-encoder-number-entry"),
				faders: boxes(".vertical-touch-fader"),
				ownClipped: [...bank.querySelectorAll("button,.vertical-touch-fader")].some((el) => {
					const box = el.getBoundingClientRect();
					const card = el.closest("article")!.getBoundingClientRect();
					return box.left < card.left - 0.5 || box.right > card.right + 0.5 || box.top < card.top - 0.5 || box.bottom > card.bottom + 0.5;
				}),
				columns: getComputedStyle(bank).gridTemplateColumns.split(" ").length,
				rows: getComputedStyle(bank).gridTemplateRows.split(" ").length,
				footerHeight: document.querySelector(".control-section")?.getBoundingClientRect().height,
				scrollWidth: bank.scrollWidth, width: bank.clientWidth,
				scrollHeight: bank.scrollHeight, height: bank.clientHeight,
			};
		});
		writeFileSync(resolve(evidence, "measurements.json"), JSON.stringify(measured, null, 2), { flag: "wx" });
		process.stdout.write(`Screen fit Main=${main} hardware=${hardware}: footer=${measured.footerHeight ?? "screen"}px, ${measured.columns} columns, ${measured.rows} rows\n`);
		expect(measured.ownClipped).toBe(false);
		expect(measured.columns).toBe(main ? 15 : 10);
		expect(measured.rows).toBe(main ? 2 : 6);
		expect(measured.cards).toHaveLength(main ? 30 : 55);
		expect(measured.scrollWidth).toBeLessThanOrEqual(measured.width + 1);
		expect(measured.scrollHeight).toBeLessThanOrEqual(measured.height + 1);
		for (const box of [...measured.cards, ...measured.actions, ...measured.faders]) {
			expect(box.left).toBeGreaterThanOrEqual(measured.bounds.left - 0.5);
			expect(box.right).toBeLessThanOrEqual(measured.bounds.right + 0.5);
			expect(box.top).toBeGreaterThanOrEqual(measured.bounds.top - 0.5);
			expect(box.bottom).toBeLessThanOrEqual(measured.bounds.bottom + 0.5);
			if (!hardware) {
				expect(box.width).toBeGreaterThanOrEqual(44);
				expect(box.height).toBeGreaterThanOrEqual(44);
			}
		}
		expect(await page.getByRole("status").count()).toBe(0);
		expect(await page.getByRole("button", { name: /Scroll Playbacks/ }).count()).toBe(0);
	} finally { await browser.close(); }
}, 15_000);

it("reports insufficient saved touch geometry and becomes operable when the area grows", async () => {
	const { browser, page } = await openFixture(false, false, 360);
	try {
		await expect.poll(() => page.getByRole("status").textContent()).toMatch(/44px touch controls/);
		expect(await page.locator(".playback-fader-bank").getAttribute("inert")).not.toBeNull();
		expect(await page.locator("article").count()).toBe(55);
		await page.getByRole("button", { name: "PAGE UP" }).click();
		expect(await page.evaluate(() => (window as unknown as { pageChanged: boolean }).pageChanged)).toBe(true);
		await page.setViewportSize({ width: 1200, height: 800 });
		await expect.poll(() => page.getByRole("status").count()).toBe(0);
		expect(await page.locator(".playback-fader-bank").getAttribute("inert")).toBeNull();
	} finally { await browser.close(); }
}, 15_000);


it("returns unused Main height to the workspace after reducing configured actions", async () => {
	const { browser, page } = await openFixture(true, false, 846);
	try {
		const before = await page.locator(".control-section").evaluate((el) => el.getBoundingClientRect().height);
		await page.evaluate(() => (window as unknown as { setActionCount: (count: number) => void }).setActionCount(1));
		await expect.poll(() => page.locator(".control-section").evaluate((el) => el.getBoundingClientRect().height)).toBeLessThan(before);
		expect(await page.locator("article").count()).toBe(30);
		expect(await page.locator(".vertical-touch-fader-actions button").count()).toBe(15);
		expect(await page.getByRole("status").count()).toBe(0);
	} finally { await browser.close(); }
}, 15_000);
