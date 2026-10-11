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
it("fits the actual Solo draft name and actions inside Main and optional command rows", async () => {
	const evidence = mkdtempSync(resolve(artifactResolver.artifactPaths.tmp, "TL714-solo-command-"));
	const contents = `import React from'react';import{createRoot}from'react-dom/client';
import{SoloCommandEditor}from${JSON.stringify(resolve(root,"apps/light-desktop/src/components/control/CommandLineBar.tsx"))};
const editor={name:'Native Solo',ready:true,available:true,selected:[{page:2,position:101},{page:4,position:102}],saving:false,setName:()=>{},save:async()=>{},cancel:()=>{}};
createRoot(document.getElementById('app')).render(<><header className="command-line-bar command-line-left"><SoloCommandEditor editor={editor}/></header><div style={{height:50}}/><header className="command-line-bar command-line-left" style={{width:600}}><SoloCommandEditor editor={editor}/></header></>);`;
	const bundle = await build({ stdin:{ contents, loader:"tsx",resolveDir:root }, bundle:true,write:false,
		outfile:resolve(evidence,"fixture.js"),platform:"browser",jsx:"automatic",logLevel:"silent",
		loader:{".svg":"dataurl",".png":"dataurl"} });
	const browser = await chromium.launch({headless:true});
	try {
		const page = await browser.newPage({viewport:{width:1440,height:500}});
		await page.setContent(`<style>${styles(resolve(root,"apps/light-desktop/src/styles.css"))}body{margin:0}</style><div id="app"></div>`);
		for (const file of bundle.outputFiles.filter(f=>f.path.endsWith('.css'))) await page.addStyleTag({content:file.text});
		for (const file of bundle.outputFiles.filter(f=>f.path.endsWith('.js'))) await page.addScriptTag({content:file.text});
		await page.getByRole('textbox',{name:'Solo Region name'}).first().waitFor();
		expect(await page.getByRole('textbox',{name:'Solo Region name'}).count()).toBe(2);
		await page.screenshot({path:resolve(evidence,'draft-row.png')});
		const rows = await page.locator('.command-line-bar').evaluateAll(elements=>elements.map(row=>{
			const bounds=row.getBoundingClientRect();
			return { height:bounds.height, children:[...row.querySelectorAll('input,button')].map(el=>{
				const r=el.getBoundingClientRect();return{top:r.top-bounds.top,bottom:r.bottom-bounds.bottom,height:r.height};
			}) };
		}));
		for (const row of rows) for (const child of row.children) {
			expect(child.top).toBeGreaterThanOrEqual(-0.5);
			expect(child.bottom).toBeLessThanOrEqual(0.5);
			expect(child.height).toBeGreaterThanOrEqual(40);
		}
	} finally { await browser.close(); }
}, 20_000);
