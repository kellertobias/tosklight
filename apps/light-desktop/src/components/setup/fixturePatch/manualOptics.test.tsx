import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import {
	manualOpticsFromWire,
	manualOpticsToWire,
} from "../../../api/manualOptics";
import { ManualOpticsFields } from "./ManualOpticsFields";
import {
	appearanceDraft,
	DEFAULT_APPEARANCE,
	normalizeAppearanceDraft,
	toPatchAppearance,
} from "./lightSourceAppearanceModel";

afterEach(cleanup);
const baseline = {
	...DEFAULT_APPEARANCE,
	manual_optics: {
		zoom_degrees: 53.5,
		shaper_blades: [0.1, 0.25, 0.5, 1] as [number, number, number, number],
	},
};

describe("installed manual optics", () => {
	it("accepts explicit null and absent legacy wire settings as inherited optics", () => {
		expect(manualOpticsFromWire({ manual_optics: null })).toEqual({});
		expect(manualOpticsFromWire({})).toEqual({});
	});

	it("preserves absent old defaults and existing manual values through source/gel edits", () => {
		const oldDraft = {
			...appearanceDraft(DEFAULT_APPEARANCE),
			luminousOutputLumens: "1200",
		};
		expect(
			normalizeAppearanceDraft(oldDraft, DEFAULT_APPEARANCE, null).appearance,
		).not.toHaveProperty("manualOptics");
		const draft = {
			...appearanceDraft(baseline),
			gelType: "custom" as const,
			customName: "Blue",
			customColor: "#1122FF",
		};
		const result = normalizeAppearanceDraft(draft, baseline, null).appearance!;
		expect(result.manualOptics).toEqual({
			zoomDegrees: 53.5,
			shaperBlades: [0.1, 0.25, 0.5, 1],
		});
		expect(toPatchAppearance(baseline).manualOptics).toEqual(result.manualOptics);
		expect(result.manualOptics!.shaperBlades).not.toBe(
			baseline.manual_optics.shaper_blades,
		);
		expect(manualOpticsToWire(manualOpticsFromWire(baseline))).toEqual({
			manual_optics: baseline.manual_optics,
		});
	});

	it("converts percentages, optional profile zoom, and valid opening boundaries", () => {
		for (const zoom of ["", "1", "170", "45.5"]) {
			const draft = {
				...appearanceDraft(DEFAULT_APPEARANCE),
				manualOpticsEdited: true,
				manualZoomDegrees: zoom,
				manualShaperBlades: ["0", "25.5", "50", "100"] as [
					string, string, string, string,
				],
			};
			const result = normalizeAppearanceDraft(draft, DEFAULT_APPEARANCE, null);
			expect(result.error).toBeUndefined();
			expect(result.appearance?.manualOptics).toEqual({
				zoomDegrees: zoom ? Number(zoom) : null,
				shaperBlades: [0, 0.255, 0.5, 1],
			});
		}
	});

	it.each(["0", "170.1", "NaN", "Infinity"])(
		"rejects invalid full zoom %s",
		(zoom) => {
			const draft = {
				...appearanceDraft(DEFAULT_APPEARANCE),
				manualOpticsEdited: true,
				manualZoomDegrees: zoom,
			};
			expect(
				normalizeAppearanceDraft(draft, DEFAULT_APPEARANCE, null).error,
			).toMatch(/Manual zoom/);
		},
	);

	it.each(["", "-1", "100.1", "NaN", "Infinity"])(
		"rejects invalid blade closure %s",
		(blade) => {
			const draft = {
				...appearanceDraft(DEFAULT_APPEARANCE),
				manualOpticsEdited: true,
			};
			draft.manualShaperBlades[2] = blade;
			expect(
				normalizeAppearanceDraft(draft, DEFAULT_APPEARANCE, null).error,
			).toMatch(/manual blade/);
		},
	);

	it("edits zoom and the exact blade through the manual control section", () => {
		function Harness() {
			const [draft, setDraft] = useState(appearanceDraft(baseline));
			return (
				<>
					<ManualOpticsFields
						draft={draft}
						update={(change) =>
							setDraft((current) => ({ ...current, ...change }))
						}
					/>
					<output data-testid="normalized">
						{JSON.stringify(
							normalizeAppearanceDraft(draft, baseline, null).appearance
								?.manualOptics,
						)}
					</output>
				</>
			);
		}
		render(<Harness />);
		expect(
			screen.getByRole("group", { name: "Manual optics" }),
		).toBeInTheDocument();
		fireEvent.change(screen.getByLabelText("Manual zoom full opening (°)"), {
			target: { value: "62" },
		});
		fireEvent.change(screen.getByLabelText("Manual blade 3 closure (%)"), {
			target: { value: "37" },
		});
		expect(
			JSON.parse(screen.getByTestId("normalized").textContent!),
		).toEqual({
			zoomDegrees: 62,
			shaperBlades: [0.1, 0.25, 0.37, 1],
		});
	});
});
