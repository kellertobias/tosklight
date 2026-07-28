import "@testing-library/jest-dom/vitest";
import { ModalProvider } from "@tosklight/ui/modals";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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

	it("renders the reviewed six-slot Curves deck on software and hardware surfaces", () => {
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

		expect(screen.getByLabelText("Curves encoders")).toHaveAttribute(
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

		expect(screen.getByLabelText("Curves encoders")).toHaveAttribute(
			"data-encoder-surface",
			"hardware",
		);
		expect(
			container.querySelectorAll(".hardware-encoder-display"),
		).toHaveLength(6);
		expect(screen.getByLabelText("Encoder 1: Top, 100%")).toBeInTheDocument();
	});

	it("puts every PWM parameter on the single Curves encoder page", () => {
		const dynamic = createDefaultDynamicDefinition(201, "intensity", {
			definition: "dynamic-201",
			lane: "lane-intensity",
		});
		dynamic.lanes[0].max_min.function = "pwm";

		render(
			<ModalProvider>
				<AppProvider>
					<DynamicDefinitionEncoderSurface
						dynamic={dynamic}
						lane={dynamic.lanes[0]}
						view="curves"
						page={1}
						onLaneChange={vi.fn().mockResolvedValue(undefined)}
						onMutate={vi.fn().mockResolvedValue(undefined)}
					/>
				</AppProvider>
			</ModalProvider>,
		);

		for (const [slot, label] of [
			[1, "Top"],
			[2, "Bottom"],
			[3, "Attack / On"],
			[4, "Decay / Off"],
			[5, "Curve width"],
			[6, "Speed"],
		] as const)
			expect(
				screen.getByRole("group", { name: `Enc ${slot} · ${label}` }),
			).toBeInTheDocument();
		expect(
			screen.getByRole("button", {
				name: "Set Enc 3 · Attack / On value",
			}),
		).toHaveTextContent("0%...50%");
		expect(
			screen.getByRole("button", {
				name: "Set Enc 4 · Decay / Off value",
			}),
		).toHaveTextContent("0%...50%");
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
		expect(screen.queryByRole("button", { name: /Red/u })).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: /Current/u }));
		const update = onLaneChange.mock.calls[0]?.[0];
		expect(update(dynamic.lanes[0]).max_min.maximum).toEqual({
			type: "current",
		});
	});

	it("edits the selected lane phase from the Phase Spread encoders", () => {
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
});
