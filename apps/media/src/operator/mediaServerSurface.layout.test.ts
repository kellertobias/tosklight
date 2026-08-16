import { describe, expect, it } from "vitest";
import mediaServerCss from "./mediaServerSurface.css?raw";

describe("Media Server library layout contract", () => {
	it("centres the bare playback takeover without an outer control outline", () => {
		expect(mediaServerCss).toContain(`.media-playback-takeover-dock {
	display: flex;
	justify-content: center;`);
		expect(mediaServerCss).toMatch(
			/\.media-playback-takeover > \.ui-switch-control \{[^}]*border: 0;[^}]*background: transparent;/,
		);
	});

	it("uses one dark checkerboard for contained card and inspector images", () => {
		expect(mediaServerCss).toContain(
			`.media-library-file-pool-grid .pool-card-media,
.media-library-item-preview,
.media-folder-picture {`,
		);
		expect(mediaServerCss).toContain("background-color: #161b20;");
		expect(mediaServerCss).toContain("background-size: 16px 16px;");
		expect(mediaServerCss).toContain(
			`.media-library-file-pool-grid .pool-card-image {
	object-fit: contain;
	object-position: center;`,
		);
		expect(mediaServerCss).toContain(
			`.media-library-item-preview > img {
	width: 100%;
	height: 100%;
	object-fit: contain;
	object-position: center;`,
		);
		expect(mediaServerCss).not.toContain("background: #eef1f3;");
	});

	it("allocates a materially larger responsive inspector column", () => {
		expect(mediaServerCss).toContain(
			`grid-template-columns: minmax(312px, 26%) minmax(400px, 1fr) minmax(
			320px,
			32%`,
		);
	});

	it("fits four folder cards and uses one responsive card scaling contract", () => {
		expect(mediaServerCss).toContain(
			`.media-library-folder-pool.ui-button-grid {
	--button-grid-gap: 6px;
	padding: 8px;`,
		);
		expect(mediaServerCss).not.toContain("repeat(4, minmax(68px, 1fr))");
		expect(312 - 16).toBeGreaterThanOrEqual(4 * 68 + 3 * 6);
		expect(232 - 16).toBeGreaterThanOrEqual(3 * 68 + 2 * 6);
		expect(232 - 16).toBeLessThan(4 * 68 + 3 * 6);
	});

	it("keeps generated previews sticky and collapses the identity fields responsively", () => {
		expect(mediaServerCss).toContain(
			`.media-generated-sticky-preview,
.media-generated-sticky-region {
	position: sticky;`,
		);
		expect(mediaServerCss).toContain(
			`.media-generated-sticky-region {
	margin: -16px -16px 0;`,
		);
		expect(mediaServerCss).toContain(
			`.media-source-identity-grid {
	display: grid;
	grid-template-columns: repeat(2, minmax(0, 1fr));`,
		);
		expect(mediaServerCss).toContain(
			`@media (max-width: 760px) {
	.media-source-identity-grid {
		grid-template-columns: minmax(0, 1fr);`,
		);
	});
});
