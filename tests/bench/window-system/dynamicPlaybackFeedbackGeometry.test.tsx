import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { build } from "esbuild";
import { chromium } from "playwright";
import { expect, it } from "vitest";
import artifactResolver from "../../../tools/artifact-paths.cjs";

const root = resolve(import.meta.dirname, "../../..");
function styles(path: string): string {
	return readFileSync(path, "utf8").replace(/@import "([^"]+)";/g, (_, name: string) =>
		styles(name === "@tosklight/ui/styles.css" ? resolve(root, "apps/ui-library/src/styles.css") : resolve(dirname(path), name)),
	);
}
const modulePath = (path: string) => JSON.stringify(resolve(root, path));
function fixture(hardware: boolean, tall: boolean) {
	return `import React from 'react';import{createRoot}from'react-dom/client';
import{HardwarePlaybackCard}from ${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/HardwarePlaybackCard.tsx")};
import{TouchPlaybackCard}from ${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/TouchPlaybackCard.tsx")};
import{dynamicPlaybackSummary}from ${modulePath("apps/light-desktop/src/components/control/playbackFaderBank/feedback.ts")};
import{playbackHasRuntimeError}from ${modulePath("apps/light-desktop/src/components/control/playbackRuntimeError.ts")};
const Card=${hardware ? "HardwarePlaybackCard" : "TouchPlaybackCard"};
const runtime={state:'active',enabled:true,controller_status:'winning',size:1,master:1,fader_value:1,effective_speed_multiplier:1,speed_source:'speed_group',target_count:1,compatible_target_count:1,lane_count:1,supported_address_count:1,missing_target_count:0,unpatched_target_count:0,instance_id:'84f05b72-bea2-4e8e-8751-d415d6a084f6',controller_id:'e380abdf-bfe9-0791-e0bd-cee977bd7241',warning:null};
function App(){const[state,setState]=React.useState('active');window.setPlaybackState=setState;window.actions=[];return <div className='screen-bank-fit'><section className='playback-fader-bank ${hardware ? "hardware" : "touch"}-layout' style={{width:1280,height:${tall ? 640 : hardware ? 176 : 320},display:'grid',gridTemplateColumns:'repeat(15,minmax(0,1fr))',gridTemplateRows:'${hardware ? "2fr 1fr" : "4fr 1fr"}'}}>{Array.from({length:30},(_,i)=>{const buttons=i>=15;const playback=buttons?null:{address:{page:3,position:i+1},name:'Beam Show',target:{type:'dynamic',assignment:{last_known_pool_number:1,fader_mode:'size_and_master'}},has_fader:true,buttons:['off','on','flash'],button_count:3};const rt={...runtime,state,enabled:state==='active',warning:state==='failed'?'No compatible targets':null};const projection={target:'dynamic',runtime:rt};const actions=buttons?[]:['OFF','ON','FLASH'].map(label=>({id:label.toLowerCase(),label,onClick:()=>window.actions.push(label)}));return <Card key={i} controller={{hardware:${hardware},activePageNumber:3,rowCount:2,state:{},runtimeActions:{},controlSourceId:'main'}} slotData={{playback,slot:i+1,rowIndex:buttons?1:0,row:{presentation:buttons?'buttons':'fader',button_count:3},cue:null,group:null}} runtimeProjection={projection} active={undefined} selected={false} kind={buttons?'empty':'dynamic'} summary={buttons?undefined:dynamicPlaybackSummary(projection)} color='#cf4aa8' hasFader={!buttons} value={100} actions={actions} touchActions={actions} className={playbackHasRuntimeError(playback,projection)?'playback-runtime-error':''} cardStyle={{gridRow:buttons?2:1,gridColumn:i%15+1}} commandTarget={null} interceptPointer={()=>{}} interceptClick={()=>{}}/>})}</section></div>};createRoot(document.getElementById('app')).render(<App/>);`;
}

it.each([[true, false], [true, true], [false, false], [false, true]])(
	"keeps Dynamic readout and controls inside all 15×2 cells: hardware=%s tall=%s",
	async (hardware, tall) => {
		const evidence = mkdtempSync(resolve(artifactResolver.artifactPaths.tmp, "TL827-geometry-"));
		const bundle = await build({ stdin: { contents: fixture(hardware, tall), loader: "tsx", resolveDir: root }, bundle: true, write: false, outfile: resolve(evidence, "fixture.js"), platform: "browser", jsx: "automatic", logLevel: "silent" });
		const browser = await chromium.launch({ headless: true });
		try {
			const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
			const css = ["styles.css", "hardware.css", "hardware-dense.css", "playback-colors.css"].map(name => styles(resolve(root, "apps/light-desktop/src", name))).join("\n");
			await page.setContent(`<style>${css}body{margin:0}</style><div id="app"></div>`);
			await page.evaluate((connected) => { document.body.className = connected ? "hardware-connected" : ""; }, hardware);
			for (const f of bundle.outputFiles) if (f.path.endsWith(".css")) await page.addStyleTag({ content: f.text });
			for (const f of bundle.outputFiles) if (f.path.endsWith(".js")) await page.addScriptTag({ content: f.text });
			await page.waitForSelector("article.playback-card");
			for (const state of ["active", "off", "pending", "failed"]) {
				await page.evaluate((next) => (window as any).setPlaybackState(next), state);
				await page.waitForFunction((next) => document.querySelector("article .playback-summary > span")?.textContent === next.toUpperCase(), state);
				const measured = await page.evaluate(() => {
					const bank = document.querySelector<HTMLElement>(".playback-fader-bank")!;
					const rect = bank.getBoundingClientRect();
					const readout = bank.querySelector(".vertical-touch-fader strong");
					const outside = (el: Element, parent: Element) => { const a = el.getBoundingClientRect(), b = parent.getBoundingClientRect(); return a.left < b.left - .5 || a.right > b.right + .5 || a.top < b.top - .5 || a.bottom > b.bottom + .5; };
					return { readoutGeometry: readout ? { rect: readout.getBoundingClientRect().toJSON(), parent: readout.parentElement!.getBoundingClientRect().toJSON(), font: getComputedStyle(readout).fontSize, bottom: getComputedStyle(readout).bottom } : null, count: bank.querySelectorAll("article").length, scrollX: bank.scrollWidth > bank.clientWidth + 1, scrollY: bank.scrollHeight > bank.clientHeight + 1, cardsOutside: [...bank.querySelectorAll("article")].filter(el => outside(el, bank)).length, controlsOutside: [...bank.querySelectorAll("button,.hardware-fader,.vertical-touch-fader")].filter(el => outside(el, el.closest("article")!)).length, readoutsClipped: [...bank.querySelectorAll(".hardware-fader > b,.vertical-touch-fader strong,.playback-summary > span,.playback-summary > small,.playback-summary-warning")].filter(el => outside(el, el.parentElement!)).map(el => el.textContent), outline: getComputedStyle(bank.querySelector("article")!).outlineColor, width: rect.width };
				});
				writeFileSync(resolve(evidence, `${state}.json`), JSON.stringify(measured, null, 2));
				await page.screenshot({ path: resolve(evidence, `${state}.png`) });
				expect(measured.count).toBe(30);
				expect(measured.scrollX || measured.scrollY).toBe(false);
				expect(measured.cardsOutside).toBe(0);
				expect(measured.controlsOutside).toBe(0);
				expect(measured.readoutsClipped).toEqual([]);
				expect(measured.outline === "rgb(255, 77, 79)").toBe(state === "failed");
			}
			await page.locator("article").first().getByRole("button", { name: "ON", exact: true }).click();
			expect(await page.evaluate(() => (window as any).actions)).toEqual(["ON"]);
		} finally { await browser.close(); }
	}, 30_000,
);
