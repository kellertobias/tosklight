import { mkdtempSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { build } from "esbuild";
import { chromium, type Page } from "playwright";
import { expect, it } from "vitest";
import artifactResolver from "../../../tools/artifact-paths.cjs";

const root = resolve(import.meta.dirname, "../../..");
const image = `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180"><rect width="110" height="180" fill="#f15b31"/><rect x="110" width="110" height="180" fill="#47de91"/><rect x="220" width="50" height="180" fill="#6599ff"/></svg>')}`;
const empty = `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180"/>')}`;
const cases = [
	{
		name: "touch fader",
		width: 64,
		height: 228,
		kind: "fader",
		hardware: false,
	},
	{
		name: "hardware fader",
		width: 140,
		height: 228,
		kind: "fader",
		hardware: true,
	},
	{
		name: "touch button",
		width: 64,
		height: 52,
		kind: "button",
		hardware: false,
	},
	{
		name: "hardware button",
		width: 140,
		height: 70,
		kind: "button",
		hardware: true,
	},
	{
		name: "touch encoder",
		width: 160,
		height: 280,
		kind: "encoder",
		hardware: false,
	},
	{
		name: "hardware encoder",
		width: 160,
		height: 280,
		kind: "encoder",
		hardware: true,
	},
];

async function pixelDifference(
	page: Page,
	first: Buffer,
	second: Buffer,
	region: { x: number; y: number; width: number; height: number },
) {
	return page.evaluate(
		async ({ a, b, region }) => {
			const pixels = async (src: string) => {
				const bitmap = await createImageBitmap(await (await fetch(src)).blob());
				const canvas = document.createElement("canvas");
				canvas.width = bitmap.width;
				canvas.height = bitmap.height;
				const context = canvas.getContext("2d");
				if (!context) throw new Error("Canvas pixels unavailable");
				context.drawImage(bitmap, 0, 0);
				return context.getImageData(0, 0, bitmap.width, bitmap.height);
			};
			const one = await pixels(a),
				two = await pixels(b);
			let changed = 0,
				total = 0;
			for (
				let y = Math.ceil(region.y);
				y < Math.floor(region.y + region.height);
				y++
			) {
				for (
					let x = Math.ceil(region.x);
					x < Math.floor(region.x + region.width);
					x++
				) {
					const offset = (y * one.width + x) * 4;
					const delta = Math.max(
						...[0, 1, 2].map((channel) =>
							Math.abs(one.data[offset + channel] - two.data[offset + channel]),
						),
					);
					if (delta > 12) changed++;
					total++;
				}
			}
			return changed / total;
		},
		{
			a: `data:image/png;base64,${first.toString("base64")}`,
			b: `data:image/png;base64,${second.toString("base64")}`,
			region,
		},
	);
}

async function artworkScreenshot(page: Page, artwork: object, path: string) {
	await page.evaluate((value) => {
		(
			window as unknown as { changeArtwork(value: unknown): void }
		).changeArtwork(value);
	}, artwork);
	const img = page.locator(".pool-card-image");
	if (await img.count())
		await img.evaluate((image: HTMLImageElement) => image.decode());
	return page.screenshot({ path });
}

it.each(
	cases,
)("keeps loaded Cue artwork visible through controls in $name", async (scenario) => {
	const evidence = mkdtempSync(
		resolve(artifactResolver.artifactPaths.tmp, "TL739-artwork-"),
	);
	const component = JSON.stringify(
		resolve(root, "apps/ui-library/src/playback/PlaybackCards.tsx"),
	);
	const applicationStyles = JSON.stringify(
		resolve(root, "apps/light-desktop/src/applicationStyles.ts"),
	);
	const contents = `import${applicationStyles};import React from'react';import{createRoot}from'react-dom/client';import{TouchPlaybackCardView,HardwarePlaybackCardView}from${component};
function App(){const[artwork,setArtwork]=React.useState({image:${JSON.stringify(image)},imageSource:'cue-preview',transparent:true});window.changeArtwork=setArtwork;window.actions=0;const onClick=()=>window.actions++;const model={page:3,slot:3,row:0,rowUnits:${scenario.kind === "button" ? 1 : 4},className:${JSON.stringify(scenario.kind === "button" ? "playback-row-compact" : "")},name:'Cuelist 3',assigned:true,kind:'cue-list',hasFader:${scenario.kind !== "button"},presentation:${JSON.stringify(scenario.kind === "encoder" ? "push_encoder" : "fader")},faderValue:100,faderLabel:'Master',faderDisplay:'100%',summary:{label:'No active Cue'},actions:${scenario.kind === "button" ? "[{id:'go',label:'GO',onClick}]" : "[{id:'go-minus',label:'GO -',onClick},{id:'go-plus',label:'GO +',onClick},{id:'flash',label:'FLASH',onClick}]"},artwork};const Card=${scenario.hardware && scenario.kind !== "encoder" ? "HardwarePlaybackCardView" : "TouchPlaybackCardView"};return <div className="screen-playback-fit screen-bank-fit"><section className="playback-fader-bank ${scenario.hardware && scenario.kind !== "encoder" ? "hardware" : "touch"}-layout" style={{width:${scenario.width},height:${scenario.height},display:'grid',gridTemplateColumns:'minmax(0,1fr)'}}><Card hardware={${scenario.hardware}} model={model}/></section></div>}createRoot(document.getElementById('app')).render(<App/>);`;
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
			viewport: { width: 400, height: 400 },
		});
		await page.setContent(
			`<style>body{margin:20px}</style><div id="app"></div>`,
		);
		for (const file of bundle.outputFiles.filter((file) =>
			file.path.endsWith(".css"),
		))
			await page.addStyleTag({ content: file.text });
		for (const file of bundle.outputFiles.filter((file) =>
			file.path.endsWith(".js"),
		))
			await page.addScriptTag({ content: file.text });
		await page
			.locator(".pool-card-image")
			.evaluate((img: HTMLImageElement) => img.decode());
		const geometry = await page.locator("article").evaluate((card) => {
			const img = card.querySelector("img");
			if (!img) throw new Error("Loaded Cue artwork absent");
			const box = img.getBoundingClientRect(),
				ratio = img.naturalWidth / img.naturalHeight;
			const width = Math.min(box.width, box.height * ratio),
				height = width / ratio;
			return {
				card: card.getBoundingClientRect().toJSON(),
				paint: {
					x: box.x + (box.width - width) / 2,
					y: box.y + (box.height - height) / 2,
					width,
					height,
				},
				fit: getComputedStyle(img).objectFit,
				buttons: [
					...card.querySelectorAll(
						".vertical-touch-fader-actions button,.faderless-playback-actions button,.single-button-playback-action,.hardware-playback-controls footer button,.touch-encoder-push,.playback-push-encoder > button",
					),
				].map((button) => button.getBoundingClientRect().toJSON()),
			};
		});
		const loaded = await page.screenshot({
			path: resolve(evidence, "loaded.png"),
		});
		await page.evaluate((src) => {
			(
				window as unknown as { changeArtwork(value: unknown): void }
			).changeArtwork({
				image: src,
				imageSource: "cue-preview",
				transparent: true,
			});
		}, empty);
		await page
			.locator(".pool-card-image")
			.evaluate((img: HTMLImageElement) => img.decode());
		const blank = await page.screenshot({
			path: resolve(evidence, "transparent.png"),
		});
		const visible = await pixelDifference(page, loaded, blank, geometry.paint);
		writeFileSync(
			resolve(evidence, "measurement.json"),
			JSON.stringify({ scenario, geometry, visible }, null, 2),
		);
		expect(
			visible,
			`Artwork content hidden by controls; evidence ${evidence}`,
		).toBeGreaterThan(0.35);
		expect(geometry.fit).toBe("contain");
		expect(
			await pixelDifference(page, loaded, blank, {
				...geometry.paint,
				x: geometry.paint.x + geometry.paint.width * 0.88,
				width: geometry.paint.width * 0.1,
			}),
		).toBe(0);
		const chosen = await artworkScreenshot(
			page,
			{ image, imageSource: "configured" },
			resolve(evidence, "configured.png"),
		);
		const chosenEmpty = await artworkScreenshot(
			page,
			{ image: empty, imageSource: "configured" },
			resolve(evidence, "configured-empty.png"),
		);
		expect(
			await pixelDifference(page, chosen, chosenEmpty, geometry.card),
		).toBeGreaterThan(0.35);
		const icon = await artworkScreenshot(
			page,
			{ icon: "★", image, imageSource: "configured" },
			resolve(evidence, "icon.png"),
		);
		expect(await page.locator(".pool-card-image").count()).toBe(0);
		const iconBox = await page
			.locator(".pool-card-icon")
			.evaluate((element) => {
				const range = document.createRange();
				range.selectNodeContents(element);
				return range.getBoundingClientRect().toJSON();
			});
		const iconEmpty = await artworkScreenshot(
			page,
			{ icon: " " },
			resolve(evidence, "icon-empty.png"),
		);
		expect(
			await pixelDifference(page, icon, iconEmpty, iconBox),
		).toBeGreaterThan(0.08);
		for (const button of geometry.buttons) {
			expect(button.top).toBeGreaterThanOrEqual(geometry.card.top - 0.5);
			expect(button.bottom).toBeLessThanOrEqual(geometry.card.bottom + 0.5);
			if (!scenario.hardware || scenario.kind === "encoder")
				expect(button.height).toBeGreaterThanOrEqual(44);
		}
		await page
			.locator(
				".vertical-touch-fader-actions button,.faderless-playback-actions button,.single-button-playback-action,.hardware-playback-controls footer button,.touch-encoder-push,.playback-push-encoder > button",
			)
			.first()
			.click();
		expect(
			await page.evaluate(
				() => (window as unknown as { actions: number }).actions,
			),
		).toBe(1);
	} finally {
		await browser.close();
	}
}, 20_000);
