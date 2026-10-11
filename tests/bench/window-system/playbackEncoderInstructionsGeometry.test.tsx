import { build } from "esbuild";
import { chromium } from "playwright";
import { mkdtempSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { expect, it } from "vitest";
import artifactResolver from "../../../tools/artifact-paths.cjs";
const root = resolve(import.meta.dirname, "../../..");
function styles(path: string): string {
	return readFileSync(path, "utf8").replace(/@import "([^"]+)";/g,
		(_, name: string) => styles(name === "@tosklight/ui/styles.css"
			? resolve(root, "apps/ui-library/src/styles.css") : resolve(dirname(path), name)));
}
it("keeps encoder instructions accessible without consuming wheel geometry on Main, optional screen and pane", async () => {
	const evidence = mkdtempSync(resolve(artifactResolver.artifactPaths.tmp, "TL714-encoder-instructions-"));
	const component = JSON.stringify(resolve(root,"apps/ui-library/src/playback/PlaybackCards.tsx"));
	const contents = `import React from'react';import{createRoot}from'react-dom/client';
import{TouchPlaybackCardView}from${component};
createRoot(document.getElementById('app')).render(<div style={{display:'flex',gap:20}}>{['main','optional','pane'].map((surface,i)=><section key={surface} aria-label={surface} className="playback-fader-bank touch-layout" style={{width:200,height:220,display:'grid',gridTemplateColumns:'minmax(0,1fr)'}}><TouchPlaybackCardView model={{page:4,slot:301+i,row:0,rowUnits:4,name:'Playback 4.'+(301+i),assigned:true,kind:'group-master',hasFader:true,presentation:'push_encoder',faderValue:100,faderLabel:'Master',faderDisplay:'100% master',summary:{label:'28 Fixtures'},status:i===1?{kind:'flash',label:'Flash'}:undefined,actions:[{id:'select',label:'SELECT'}]}}/></section>)}</div>);`;
	const bundle = await build({stdin:{contents,loader:'tsx',resolveDir:root},bundle:true,write:false,
		outfile:resolve(evidence,'fixture.js'),platform:'browser',jsx:'automatic',logLevel:'silent'});
	const browser = await chromium.launch({headless:true});
	try {
		const page = await browser.newPage({viewport:{width:900,height:400}});
		await page.setContent(`<style>${styles(resolve(root,'apps/light-desktop/src/styles.css'))}body{margin:0}</style><div id="app"></div>`);
		for(const file of bundle.outputFiles.filter(f=>f.path.endsWith('.css')))await page.addStyleTag({content:file.text});
		for(const file of bundle.outputFiles.filter(f=>f.path.endsWith('.js')))await page.addScriptTag({content:file.text});
		await page.locator('.touch-encoder').first().waitFor();
		await page.screenshot({path:resolve(evidence,'encoders.png')});
		const measurements = await page.locator('.touch-encoder').evaluateAll(elements=>elements.map(encoder=>{
			const instructions=encoder.querySelector<HTMLElement>('.visually-hidden')!;
			const card=encoder.closest('article')!.getBoundingClientRect();
			const surface=encoder.querySelector('.touch-encoder-surface')!.getBoundingClientRect();
			const wheel = encoder.querySelector('.touch-encoder-surface')!;
			const ring = getComputedStyle(wheel, '::before');
			const center = encoder.querySelector('.touch-encoder-push')!;
			const centerBox = center.getBoundingClientRect();
			const buttons=[...encoder.querySelectorAll('button')].map(button=>button.getBoundingClientRect().toJSON());
			return {instructionsPosition:getComputedStyle(instructions).position,
				instructionsWidth:instructions.getBoundingClientRect().width,
				description:encoder.getAttribute('aria-describedby'),instructionId:instructions.id,
				text:instructions.textContent,card:card.toJSON(),surface:surface.toJSON(),buttons,
				ringWidth:parseFloat(ring.width),ringHeight:parseFloat(ring.height),
				ringBackground:ring.backgroundImage,ringRadius:ring.borderRadius,
				center: centerBox.toJSON(),centerText:center.textContent,
				rotaryPosition:encoder.closest('.playback-rotary-control')!.getAttribute('data-position')};
		}));
		expect(measurements).toHaveLength(3);
		for(const item of measurements){
			expect(item.instructionsPosition).toBe('absolute');
			expect(item.instructionsWidth).toBeLessThanOrEqual(1);
			expect(item.description).toBe(item.instructionId);
			expect(item.text).toContain('Press the center for the configured Playback action');
			expect(item.surface.height).toBeGreaterThanOrEqual(88);
			expect(item.ringBackground).toContain('conic-gradient');
			expect(item.ringWidth).toBeGreaterThanOrEqual(52);
			expect(item.ringHeight).toBeCloseTo(item.ringWidth, 1);
			expect(item.ringRadius).toBe('50%');
			expect(item.ringWidth).toBeLessThanOrEqual(item.surface.width);
			expect(item.ringHeight).toBeLessThanOrEqual(item.surface.height);
			expect(item.center.width).toBeGreaterThanOrEqual(44);
			expect(item.centerText).toContain('100%');
			expect(item.rotaryPosition).toBe('100');
			for(const button of item.buttons){
				expect(button.height).toBeGreaterThanOrEqual(44);
				expect(button.top).toBeGreaterThanOrEqual(item.card.top-0.5);
				expect(button.bottom).toBeLessThanOrEqual(item.card.bottom+0.5);
			}
		}
	}finally{await browser.close();}
},20_000);
