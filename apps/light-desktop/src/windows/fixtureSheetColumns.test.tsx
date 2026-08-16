import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { fixtureSheetColumns } from "./fixtureSheetColumns";
import type { FixtureSheetRow } from "./fixtureSheetProjection";
import type { FixtureStepPresenter } from "./fixtureSheetStep";

const present: FixtureStepPresenter = () => ({
	current: false,
	containedCurrent: false,
	base: false,
	containedBase: false,
});

function row(overrides: Partial<FixtureSheetRow>): FixtureSheetRow {
	return {
		id: 1,
		name: "Front Frost 1",
		type: "Fresnel",
		fixtureType: "Generic · Fresnel",
		patch: "U1.1",
		icon: null,
		fixtureId: "fixture-1",
		targetKind: "fixture",
		parentFixtureId: "fixture-1",
		childFixtureIds: [],
		indented: false,
		dimmer: 0,
		color: "transparent",
		colorAvailable: false,
		colorLabel: "—",
		pan: 50,
		tilt: 50,
		positionAvailable: false,
		positionLabel: "—",
		preloadDimmer: null,
		preloadColor: null,
		preloadPan: null,
		preloadTilt: null,
		beam: "—",
		focus: "—",
		limitingGroups: [],
		sources: {
			dimmer: "default",
			color: "default",
			position: "default",
			beam: "default",
			focus: "default",
		},
		...overrides,
	} as FixtureSheetRow;
}

function renderColumn(id: string, fixture: FixtureSheetRow) {
	const column = fixtureSheetColumns(false, present).find(
		(candidate) => candidate.id === id,
	);
	if (!column?.render) throw new Error(`the ${id} column has no renderer`);
	return render(column.render(fixture, 0)).container;
}

afterEach(cleanup);

describe("Fixture Sheet colour and position previews", () => {
	it("shows no preview for a lantern without the attribute", () => {
		const lantern = row({});

		const color = renderColumn("color", lantern);
		const position = renderColumn("position", lantern);

		expect(color.querySelector(".color-dot")).toBeNull();
		expect(color.textContent).toContain("—");
		expect(position.querySelector(".position-glyph")).toBeNull();
		expect(position.textContent).toContain("—");
	});

	it("keeps the preview for a lantern that carries the attribute", () => {
		const lantern = row({
			color: "rgb(255, 255, 255)",
			colorAvailable: true,
			colorLabel: "100% / 100% / 100%",
			positionAvailable: true,
			positionLabel: undefined,
		});

		const color = renderColumn("color", lantern);
		const position = renderColumn("position", lantern);

		expect(color.querySelector(".color-dot")).not.toBeNull();
		expect(position.querySelector(".position-glyph")).not.toBeNull();
		expect(position.textContent).toContain("50");
	});

	it("distinguishes full, partial, and contained Freeze state", () => {
		const full = renderColumn(
			"name",
			row({ freeze: { full: true, families: [], contained: false } }),
		);
		expect(full.textContent).toContain("❄ FREEZE");
		expect(
			full.querySelector(".fixture-freeze-status")?.getAttribute("title"),
		).toContain("ignores all controls");

		const partial = renderColumn(
			"name",
			row({
				freeze: {
					full: false,
					families: ["intensity", "color"],
					contained: true,
				},
			}),
		);
		expect(partial.textContent).toContain(
			"❄ FREEZE · Intensity · Color INSIDE",
		);
		expect(
			partial.querySelector(".fixture-freeze-status")?.getAttribute("title"),
		).toContain("fixture heads");
	});
});

describe("Fixture Sheet attribute value fitting", () => {
	it("keeps Beam members as individually clipped one-line values", () => {
		const beam = renderColumn(
			"beam",
			row({
				groupValues: {
					beam: {
						id: "beam",
						available: true,
						source: "programmer",
						accessibleName: "Beam, Prism 8 facet, Prism rotation 44%",
						members: [
							{
								attribute: "prism.prism",
								label: "Prism",
								value: { kind: "indexed", index: 1 },
								text: "8 facet",
								preloadValue: null,
								preloadText: null,
								source: "programmer",
								dynamics: [],
							},
							{
								attribute: "prism.prism_rotation",
								label: "Prism rotation",
								value: { kind: "normalized", value: 0.44 },
								text: "44%",
								preloadValue: null,
								preloadText: null,
								source: "programmer",
								dynamics: [],
							},
						],
					},
				} as unknown as FixtureSheetRow["groupValues"],
			}),
		);

		const presentation = beam.querySelector(
			".fixture-sheet-multi-value-presentation",
		);
		const members = presentation?.querySelectorAll(
			":scope > .fixture-sheet-member-value",
		);
		expect(presentation).toHaveAttribute(
			"aria-label",
			"Beam, Prism 8 facet, Prism rotation 44%",
		);
		expect(members).toHaveLength(2);
		expect(members?.[0]).toHaveAttribute("title", "Prism 8 facet");
		expect(members?.[1]).toHaveAttribute("title", "Prism rotation 44%");
		expect(presentation).toHaveTextContent("Prism 8 facet");
		expect(presentation).toHaveTextContent("Prism rotation 44%");
	});
});
