import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { chromium } from "playwright";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { TouchPlaybackCardView } from "../../../apps/ui-library/src/playback/PlaybackCards";

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

it.each([
	4, 2,
])("keeps native chrome, five assigned push encoder controls and matrix reachable with %ifr encoder tracks", async (rowUnits) => {
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage({
			viewport: { width: 1200, height: 800 },
		});
		const cards = Array.from({ length: 5 }, (_, index) => (
			<TouchPlaybackCardView
				key={index}
				model={{
					page: 4,
					slot: 301 + index,
					row: 0,
					rowUnits,
					name: "Group Master",
					assigned: true,
					hasFader: true,
					presentation: "push_encoder",
					faderValue: 100,
					faderLabel: "Master",
					faderDisplay: "100% master",
					summary: { label: "28 Fixtures", detail: "100%" },
					actions: [{ id: "select", label: "SELECT" }],
					style: { gridRow: 1, gridColumn: index + 1 },
				}}
			/>
		));
		const matrix = Array.from({ length: 50 }, (_, index) => (
			<TouchPlaybackCardView
				key={index}
				model={{
					page: 4,
					slot: 201 + index,
					row: 1 + Math.floor(index / 10),
					rowUnits: 1,
					name: "Matrix",
					assigned: true,
					hasFader: false,
					presentation: "buttons",
					className: "playback-row-compact",
					faderValue: 0,
					faderLabel: "Master",
					faderDisplay: "OFF",
					actions: [{ id: "go", label: "GO" }],
					style: {
						gridRow: 2 + Math.floor(index / 10),
						gridColumn: 1 + (index % 10),
					},
				}}
			/>
		));
		const markup = renderToStaticMarkup(
			<div className="screen-shell utility-content">
				<div className="native-drag-strip">
					<button>X</button>
					<button>Fullscreen</button>
					<button>Move</button>
				</div>
				<section className="screen-playbacks playbacks-only">
					<section className="playback-surface-controls">
						<div>
							<div className="playback-solo-toolbar">
								<button>Select Solo members</button>
							</div>
						</div>
						<div className="playback-bank-fixed">
							<div
								className="playback-fader-bank touch-layout"
								style={{
									gridTemplateColumns: "repeat(10,minmax(0,1fr))",
									gridTemplateRows: `${rowUnits}fr repeat(5,1fr)`,
								}}
							>
								{cards}
								{matrix}
							</div>
						</div>
					</section>
				</section>
			</div>,
		);
		await page.setContent(
			`<style>${styles("apps/light-desktop/src/styles.css")}body{margin:0;font:14px sans-serif}*{box-sizing:border-box}</style>${markup}`,
		);
		const rectangles = await page.evaluate(() => {
			const native = document
				.querySelector(".native-drag-strip")!
				.getBoundingClientRect();
			const toolbar = document
				.querySelector(".playback-solo-toolbar")!
				.getBoundingClientRect();
			return {
				nativeBottom: native.bottom,
				matrix: [
					...document.querySelectorAll(".single-button-playback-action"),
				].map((button) => {
					const box = button.getBoundingClientRect();
					return { top: box.top, bottom: box.bottom, height: box.height };
				}),
				toolbarTop: toolbar.top,
				cards: [
					...document.querySelectorAll("article:has(.playback-push-encoder)"),
				].map((card) => {
					const box = card.getBoundingClientRect();
					const action = card
						.querySelector(".touch-encoder-push")!
						.getBoundingClientRect();
					const value = card.querySelector(".touch-encoder-value")!;
					const rotary = card.querySelector(".playback-rotary-control")!;
					const ring = getComputedStyle(card.querySelector(".touch-encoder-surface")!, "::before");
					const numeric = card.querySelector(".touch-encoder-number-entry")!.getBoundingClientRect();
					return {
						bottom: box.bottom,
						position: rotary.getAttribute("data-position"),
						ring: ring.backgroundImage,
						actionWidth: action.width,
						actionLeft: action.left,
						actionRight: action.right,
						cardLeft: box.left,
						cardRight: box.right,
						actionTop: action.top,
						actionBottom: action.bottom,
						actionHeight: action.height,
						numericHeight: numeric.height,
						numericWidth: numeric.width,
						numericBottom: numeric.bottom,
						valueFits: value.scrollWidth <= value.clientWidth,
					};
				}),
			};
		});
		expect(rectangles.cards).toHaveLength(5);
		expect(rectangles.matrix).toHaveLength(50);
		for (const button of rectangles.matrix) {
			expect(button.height).toBeGreaterThanOrEqual(44);
			expect(button.bottom).toBeLessThanOrEqual(800);
		}
		for (const card of rectangles.cards) {
			expect(card.actionBottom).toBeLessThanOrEqual(card.bottom);
			expect(card.actionHeight).toBeGreaterThanOrEqual(44);
			expect(card.actionWidth).toBeGreaterThanOrEqual(44);
			expect(card.actionLeft).toBeGreaterThanOrEqual(card.cardLeft);
			expect(card.actionRight).toBeLessThanOrEqual(card.cardRight);
			expect(card.position).toBe("100");
			expect(card.ring).toContain("conic-gradient");
			expect(card.numericHeight).toBeGreaterThanOrEqual(44);
			expect(card.numericWidth).toBeGreaterThanOrEqual(44);
			expect(card.numericBottom).toBeLessThanOrEqual(card.bottom);
			expect(card.valueFits).toBe(true);
		}
		expect(rectangles.toolbarTop).toBeGreaterThanOrEqual(
			rectangles.nativeBottom,
		);
	} finally {
		await browser.close();
	}
}, 15_000);
