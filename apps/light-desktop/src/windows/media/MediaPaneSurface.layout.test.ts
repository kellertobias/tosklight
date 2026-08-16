import { describe, expect, it } from "vitest";
import mediaPaneCss from "./MediaPaneSurface.css?raw";

describe("Media pane layout contract", () => {
	it("makes the master picture edge-to-edge while keeping info padding separate", () => {
		expect(mediaPaneCss).toContain(
			`.media-composite-frame.ui-button {
	width: 100%;
	min-width: 0;
	min-height: 50px;
	margin: 0;
	padding: 0;
	gap: 0;`,
		);
		expect(mediaPaneCss).toContain(
			`.media-composite-picture {
	position: relative;
	width: 100%;
	max-width: none;`,
		);
		expect(mediaPaneCss).toContain(
			`.media-composite-info {
	display: grid;`,
		);
		expect(mediaPaneCss).toContain("padding: 6px 7px;");
	});

	it("keeps secondary controls in the right grid column", () => {
		expect(mediaPaneCss).toContain(
			`.media-pane-workspace > .media-secondary-controls:not(:only-child) {
\tgrid-column: 2;
}`,
		);
		expect(mediaPaneCss).not.toContain(
			`.media-secondary-controls {
\t\tdisplay: none;`,
		);
	});

	it("reserves one complete folder-card row", () => {
		expect(mediaPaneCss).toContain(
			"--media-folder-card-size: clamp(104px, 12cqh, 120px);",
		);
		expect(mediaPaneCss).toContain(
			"flex: 0 0 calc(var(--media-folder-card-size) + 8px);",
		);
		expect(mediaPaneCss).toContain(
			"min-height: calc(var(--media-folder-card-size) + 8px);",
		);
		expect(mediaPaneCss).toContain(
			`.media-folder-pool-grid .pool-card {
\twidth: var(--media-folder-card-size);
\theight: var(--media-folder-card-size);
}`,
		);
	});

	it("uses one mask accent for folder and file cards without changing geometry", () => {
		expect(mediaPaneCss).toContain(
			`.media-library-browser.is-mask {
	--media-mask-accent: #d84b9b;`,
		);
		expect(mediaPaneCss).toContain(
			`.media-library-browser.is-mask .pool-card:not([data-pool-slot-id="0"]) {
	--pool-card-color: var(--media-mask-accent) !important;
	--pool-card-uncolored-color: var(--media-mask-accent);`,
		);
		expect(mediaPaneCss).not.toContain(
			`.media-library-browser.is-mask .media-folder-pool-grid`,
		);
	});

	it("visually separates the 000 clear entry from unavailable placeholders", () => {
		expect(mediaPaneCss).toContain(
			`.media-file-pool-grid [data-pool-slot-id="0"] {
	--pool-card-color: #788b96 !important;`,
		);
	});
});
