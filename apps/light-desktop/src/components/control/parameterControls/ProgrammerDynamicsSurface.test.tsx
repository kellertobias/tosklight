import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppProvider } from "../../../state/AppContext";
import { createDefaultDynamicDefinition } from "../../../windows/DynamicsWindow";
import { DynamicDefinitionEncoderSurface } from "./ProgrammerDynamicsSurface";

let hardwareConnected = false;

vi.mock("../../../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show-test",
	useHardwareConnected: () => hardwareConnected,
}));

describe("DynamicDefinitionEncoderSurface", () => {
	afterEach(cleanup);
	beforeEach(() => {
		hardwareConnected = false;
		Object.defineProperty(window, "localStorage", {
			configurable: true,
			value: {
				getItem: vi.fn(() => null),
				setItem: vi.fn(),
				removeItem: vi.fn(),
				clear: vi.fn(),
			},
		});
	});

	it("renders the reviewed six-slot Lanes deck on software and hardware surfaces", () => {
		const dynamic = createDefaultDynamicDefinition(201, "intensity", {
			definition: "dynamic-201",
			lane: "lane-intensity",
		});
		const props = {
			dynamic,
			lane: dynamic.lanes[0] ?? null,
			view: "curves" as const,
			page: 1,
			onLaneChange: vi.fn().mockResolvedValue(undefined),
			onMutate: vi.fn().mockResolvedValue(undefined),
		};
		const { container, rerender } = render(
			<ModalProvider>
				<AppProvider>
					<DynamicDefinitionEncoderSurface {...props} />
				</AppProvider>
			</ModalProvider>,
		);

		expect(screen.getByLabelText("Lanes encoders")).toHaveAttribute(
			"data-encoder-surface",
			"touch",
		);
		expect(container.querySelectorAll(".touch-encoder")).toHaveLength(6);
		expect(
			screen.getByRole("group", { name: "Enc 1 · Top" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("group", { name: "Enc 5 · Curve width" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("group", { name: "Enc 6 · Speed" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("group", { name: /Function/ }),
		).not.toBeInTheDocument();

		hardwareConnected = true;
		rerender(
			<ModalProvider>
				<AppProvider>
					<DynamicDefinitionEncoderSurface {...props} />
				</AppProvider>
			</ModalProvider>,
		);

		expect(screen.getByLabelText("Lanes encoders")).toHaveAttribute(
			"data-encoder-surface",
			"hardware",
		);
		expect(
			container.querySelectorAll(".hardware-encoder-display"),
		).toHaveLength(6);
		expect(screen.getByLabelText("Encoder 1: Top, 100%")).toBeInTheDocument();
	});

	it("puts every PWM parameter on the single Lanes encoder page", () => {
		const dynamic = createDefaultDynamicDefinition(201, "intensity", {
			definition: "dynamic-201",
			lane: "lane-intensity",
		});
		dynamic.lanes[0].max_min.function = "pwm";
		dynamic.lanes[0].width = 0.4;
		const onLaneChange = vi.fn().mockResolvedValue(undefined);

		render(
			<ModalProvider>
				<AppProvider>
					<DynamicDefinitionEncoderSurface
						dynamic={dynamic}
						lane={dynamic.lanes[0]}
						view="curves"
						page={1}
						onLaneChange={onLaneChange}
						onMutate={vi.fn().mockResolvedValue(undefined)}
					/>
				</AppProvider>
			</ModalProvider>,
		);

		for (const [slot, label] of [
			[1, "Top"],
			[2, "Bottom"],
			[3, "Attack"],
			[4, "On"],
			[5, "Decay"],
			[6, "Speed"],
		] as const)
			expect(
				screen.getByRole("group", { name: `Enc ${slot} · ${label}` }),
			).toBeInTheDocument();
		expect(
			screen.getByRole("button", {
				name: "Set Enc 3 · Attack value",
			}),
		).toHaveTextContent("0%");
		expect(
			screen.getByRole("button", {
				name: "Set Enc 4 · On value",
			}),
		).toHaveTextContent("50%");
		expect(
			screen.getByRole("button", {
				name: "Set Enc 5 · Decay value",
			}),
		).toHaveTextContent("0%");
		expect(
			screen.queryByRole("group", { name: /Curve width/ }),
		).not.toBeInTheDocument();
		fireEvent.keyDown(screen.getByRole("group", { name: "Enc 4 · On" }), {
			key: "ArrowUp",
		});
		const update = onLaneChange.mock.calls[0]?.[0];
		const updated = update(dynamic.lanes[0]);
		expect(updated.width).toBe(1);
		expect(updated.max_min.pwm.on).toBeCloseTo(0.51);
		expect(updated.max_min.pwm.off).toBeCloseTo(0.49);
		expect(
			screen.queryByRole("group", { name: /Function/ }),
		).not.toBeInTheDocument();
	});

	it("offers Current first and applies attribute-compatible preset sources", () => {
		const dynamic = createDefaultDynamicDefinition(201, "intensity", {
			definition: "dynamic-201",
			lane: "lane-intensity",
		});
		const onLaneChange = vi.fn().mockResolvedValue(undefined);

		render(
			<ModalProvider>
				<AppProvider>
					<DynamicDefinitionEncoderSurface
						dynamic={dynamic}
						presets={[
							{
								kind: "preset",
								id: "1.4",
								revision: 2,
								updated_at: "2026-07-28T00:00:00Z",
								body: {
									name: "Half",
									number: 4,
									family: "Intensity",
									values: { fixture: { intensity: 0.5 } },
								},
							},
							{
								kind: "preset",
								id: "2.1",
								revision: 1,
								updated_at: "2026-07-28T00:00:00Z",
								body: {
									name: "Red",
									number: 1,
									family: "Color",
									values: { fixture: { "color.red": 1 } },
								},
							},
						]}
						lane={dynamic.lanes[0]}
						view="curves"
						onLaneChange={onLaneChange}
						onMutate={vi.fn().mockResolvedValue(undefined)}
					/>
				</AppProvider>
			</ModalProvider>,
		);

		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Top value" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Show presets" }));
		expect(screen.getByRole("button", { name: /Current/u })).toHaveAttribute(
			"aria-pressed",
			"false",
		);
		expect(screen.getByRole("button", { name: /Half/u })).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /Red/u }),
		).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: /Current/u }));
		const update = onLaneChange.mock.calls[0]?.[0];
		expect(update(dynamic.lanes[0]).max_min.maximum).toEqual({
			type: "current",
		});
	});

	it("edits the selected lane phase from the Phase encoders", () => {
		const dynamic = createDefaultDynamicDefinition(201, "intensity", {
			definition: "dynamic-201",
			lane: "lane-intensity",
		});
		const lane = dynamic.lanes[0];
		dynamic.phase_mode = "per_lane";
		lane.phase = { ...dynamic.phase, span_degrees: 180 };
		const onLaneChange = vi.fn().mockResolvedValue(undefined);
		const onMutate = vi.fn().mockResolvedValue(undefined);

		render(
			<ModalProvider>
				<AppProvider>
					<DynamicDefinitionEncoderSurface
						dynamic={dynamic}
						lane={lane}
						view="phase"
						onLaneChange={onLaneChange}
						onMutate={onMutate}
					/>
				</AppProvider>
			</ModalProvider>,
		);

		expect(
			screen.getByRole("button", { name: "Set Enc 2 · Span value" }),
		).toHaveTextContent("180°");
		fireEvent.keyDown(screen.getByRole("group", { name: "Enc 2 · Span" }), {
			key: "ArrowUp",
		});

		const update = onLaneChange.mock.calls[0]?.[0];
		expect(update(lane).phase?.span_degrees).toBe(185);
		expect(onMutate).not.toHaveBeenCalled();
	});

	it("accepts an explicit THRU range from Offset and Span encoders", () => {
		const dynamic = createDefaultDynamicDefinition(201, "intensity", {
			definition: "dynamic-201",
			lane: "lane-intensity",
		});
		const lane = dynamic.lanes[0];
		const onLaneChange = vi.fn().mockResolvedValue(undefined);
		const onMutate = vi.fn().mockResolvedValue(undefined);

		render(
			<ModalProvider>
				<AppProvider>
					<DynamicDefinitionEncoderSurface
						dynamic={dynamic}
						lane={lane}
						view="phase"
						onLaneChange={onLaneChange}
						onMutate={onMutate}
					/>
				</AppProvider>
			</ModalProvider>,
		);

		for (const { encoder, keys, expected } of [
			{
				encoder: "Offset",
				keys: ["0", "THRU", "3", "6", "0", "ENTER"],
				expected: {
					offset_degrees: 0,
					span_degrees: 360,
					anchors_degrees: [0, 360],
				},
			},
			{
				encoder: "Span",
				keys: ["3", "6", "0", "THRU", "0", "ENTER"],
				expected: {
					offset_degrees: 360,
					span_degrees: -360,
					anchors_degrees: [0, -360],
				},
			},
		]) {
			fireEvent.click(
				screen.getByRole("button", {
					name: new RegExp(`Set Enc \\d · ${encoder} value`, "u"),
				}),
			);
			for (const key of keys)
				fireEvent.click(screen.getByRole("button", { name: key }));
			const update = onMutate.mock.calls.at(-1)?.[0];
			if (!update) throw new Error(`Missing ${encoder} range update`);
			expect(update.type).toBe("set_phase");
			if (update.type !== "set_phase")
				throw new Error(`Unexpected ${encoder} range update`);
			expect(update.phase).toMatchObject(expected);
		}
		expect(onLaneChange).not.toHaveBeenCalled();
	});
});
