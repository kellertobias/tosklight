import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { describe, expect, it } from "vitest";
import { chromium } from "playwright";
import { renderToStaticMarkup } from "react-dom/server";
import { PaneView } from "../../../apps/ui-library/src/desktop/PaneView";
import { TouchPlaybackCardView } from "../../../apps/ui-library/src/playback/PlaybackCards";
import { scrollablePlaybackGeometry } from "../../../apps/light-desktop/src/components/control/playbackFaderBank/scrollableGeometry";
const pane = {
	id: "pane",
	title: "Playbacks",
	x: 13,
	y: 1,
	width: 12,
	height: 18,
};
const geometry = scrollablePlaybackGeometry(
	Array.from({ length: 14 }, () => "minmax(0, 1fr)").join(" "),
	20,
);
function content(maximized: boolean) {
	const cards = Array.from({ length: 280 }, (_, index) => (
		<TouchPlaybackCardView
			key={index}
			model={{
				page: 1,
				slot: 401 + index,
				row: Math.floor(index / 20),
				rowUnits: 1,
				name: "Blue Pulse",
				assigned: true,
				kind: "dynamic",
				className: "playback-row-compact",
				presentation: "buttons",
				hasFader: false,
				faderValue: 0,
				faderLabel: "Master",
				faderDisplay: "OFF",
				actions: [{ id: "go", label: "OFF" }],
			}}
		/>
	));
	return renderToStaticMarkup(
		<div className="desk-grid">
			<PaneView pane={pane} maximized={maximized}>
				<section className="playback-pane">
					<section className="playback-surface-controls">
						<div className="playback-solo-toolbar">
							Select Solo members · Manage Solo Regions
						</div>
						<div className="playback-bank-viewport">
							<div
								className="playback-fader-bank touch-layout"
								style={geometry}
							>
								{cards}
							</div>
						</div>
					</section>
				</section>
			</PaneView>
		</div>,
	);
}
function styles(path: string): string {
	return readFileSync(path, "utf8").replace(
		/@import "([^"]+)";/g,
		(_, name: string) =>
			styles(
				name === "@tosklight/ui/styles.css"
					? "apps/ui-library/src/styles.css"
					: resolve(dirname(path), name),
			),
	);
}
const css = styles("apps/light-desktop/src/styles.css");

describe("rendered Playback pane geometry", () => {
	it("preserves readable20×14 touch cells when normal, maximized and restored", async () => {
		const browser = await chromium.launch({ headless: true });
		try {
			const page = await browser.newPage({
				viewport: { width: 1280, height: 900 },
			});
			const measurements = [];
			for (const maximized of [false, true, false]) {
				await page.setContent(
					`<style>${css}\nbody{margin:0;font:14px sans-serif}.desk-grid{width:1200px;height:700px}*{box-sizing:border-box}</style>${content(maximized)}<p>Browser geometry verification: ${maximized ? "maximized" : measurements.length === 0 ? "normal half-width" : "restored half-width"} pane. Fixed20×14 positions401–680; readable112×92px minimum cells. Scroll to reveal remaining positions.</p>`,
				);
				const result = await page.evaluate(() => {
					const rect = (selector: string) =>
						document.querySelector(selector)!.getBoundingClientRect().toJSON();
					const viewport = document.querySelector(".playback-bank-viewport")!;
					const button = document.querySelector(
						".single-button-playback-action",
					)!;
					const identity = button.querySelector("strong")!;
					const name = button.querySelector(
						".single-button-playback-identity > span",
					)!;
					return {
						pane: rect(".desk-pane"),
						cell: rect(".touch-playback-card"),
						action: rect(".single-button-playback-action"),
						address: identity.getBoundingClientRect().toJSON(),
						name: name.getBoundingClientRect().toJSON(),
						viewport: rect(".playback-bank-viewport"),
						scrollWidth: viewport.scrollWidth,
						scrollHeight: viewport.scrollHeight,
						cellRects: [
							...document.querySelectorAll(".touch-playback-card"),
						].map((el) => el.getBoundingClientRect().toJSON()),
						addresses: [
							...document.querySelectorAll(
								".single-button-playback-identity strong",
							),
						].map((el) => el.textContent),
					};
				});
				expect(result.addresses).toEqual(
					Array.from({ length: 280 }, (_, i) => `1.${401 + i}`),
				);
				expect(result.cellRects[19].y).toBe(result.cellRects[0].y);
				expect(result.cellRects[20].x).toBe(result.cellRects[0].x);
				expect(result.cellRects[20].y).toBeGreaterThan(result.cellRects[0].y);
				expect(result.cell.width).toBeGreaterThanOrEqual(112);
				expect(result.cell.height).toBeGreaterThanOrEqual(92);
				expect(result.action.height).toBeGreaterThanOrEqual(44);
				expect(result.address.bottom).toBeLessThanOrEqual(result.name.top);
				expect(result.viewport.height).toBeGreaterThan(300);
				expect(result.viewport.bottom).toBeLessThanOrEqual(result.pane.bottom);
				expect(result.cell.y).toBeGreaterThanOrEqual(result.viewport.y);
				expect(result.cell.y).toBeLessThan(result.viewport.bottom);
				expect(result.scrollWidth).toBeGreaterThan(result.viewport.width);
				expect(result.scrollHeight).toBeGreaterThan(result.viewport.height);
				await page.locator(".playback-bank-viewport").hover();
				await page.mouse.wheel(0, 4000);
				await page.mouse.wheel(1900, 0);
				await page.waitForFunction(
					() => {
						const viewport = document.querySelector(".playback-bank-viewport")!;
						return viewport.scrollTop > 0 && viewport.scrollLeft > 0;
					},
					undefined,
					{ timeout: 2000 },
				);
				const endVisible = await page.evaluate(() => {
					const viewport = document
						.querySelector(".playback-bank-viewport")!
						.getBoundingClientRect();
					const last = [...document.querySelectorAll(".touch-playback-card")]
						.at(-1)!
						.getBoundingClientRect();
					return (
						last.top >= viewport.top &&
						last.bottom <= viewport.bottom &&
						last.left >= viewport.left &&
						last.right <= viewport.right
					);
				});
				expect(endVisible).toBe(true);
				await page.screenshot({
					path: `.artifacts/tmp/TL714-pane-browser-${measurements.length}.png`,
				});
				measurements.push(result.pane);
			}
			expect(measurements[0].width).toBeLessThan(610);
			expect(measurements[1].width).toBe(1200);
			expect(measurements[1].x).toBe(0);
			expect(measurements[2]).toEqual(measurements[0]);
		} finally {
			await browser.close();
		}
	}, 15000);
});
