import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	HardwareCueRowsView,
	HardwarePlaybackCardView,
	hardwarePickupGeometry,
	PlaybackBankView,
	type PlaybackCardViewModel,
	TouchPlaybackCardView,
} from "./PlaybackCards";

const model: PlaybackCardViewModel = {
	page: 2,
	slot: 3,
	row: 0,
	rowUnits: 2,
	name: "Main Cuelist",
	assigned: true,
	selected: true,
	hasFader: true,
	faderValue: 62,
	faderLabel: "Main Cuelist",
	faderDisplay: "62%",
	faderMode: "Cue 4",
	actions: [
		{ id: "go", label: "GO" },
		{ id: "off", label: "OFF" },
	],
};

afterEach(cleanup);

describe("playback card views", () => {
	it("emits touch fader changes without owning playback services", () => {
		const change = vi.fn();
		render(
			<TouchPlaybackCardView
				model={model}
				callbacks={{ onFaderChange: change }}
			/>,
		);
		const fader = screen.getByRole("slider", { name: "Main Cuelist" });
		fireEvent.input(fader, { target: { value: "70" } });
		fireEvent.pointerUp(fader);
		expect(change).toHaveBeenCalledWith(70);
	});

	it("preserves explicit hardware cue and fader presentation", () => {
		render(
			<HardwarePlaybackCardView
				model={model}
				cueRows={
					<HardwareCueRowsView
						previous={{ number: 3, name: "Look" }}
						current={{ number: 4, name: "Solo", fadeMillis: 2500 }}
						next={{ number: 5, name: "Blackout" }}
						progress={0.5}
					/>
				}
			/>,
		);
		expect(screen.getByText("2.3")).toBeInTheDocument();
		expect(screen.getByText("Solo")).toBeInTheDocument();
		expect(
			screen.getByRole("slider", { name: "Page 2 playback 3 fader" }),
		).toHaveValue("62");
	});

	it("renders the hardware-only difference between physical position and target", () => {
		const rendered = render(
			<HardwarePlaybackCardView
				model={{
					...model,
					faderValue: 25,
					faderDisplay: "25%",
					hardwarePickup: {
						physicalPosition: 0.25,
						pickupTarget: 0.75,
					},
				}}
			/>,
		);
		const fader = document.querySelector(".hardware-fader");
		expect(fader).toHaveAttribute("data-pickup-direction", "raise");
		expect(fader).toHaveAttribute("data-pickup-physical", "0.25");
		expect(fader).toHaveAttribute("data-pickup-target", "0.75");
		expect(fader).toHaveStyle({
			"--hardware-pickup-start": "25%",
			"--hardware-pickup-size": "50%",
		});
		expect(
			document.querySelector(".hardware-fader-pickup-difference"),
		).toBeInTheDocument();
		expect(screen.getByText("Physical 25% · Target 75%")).toBeInTheDocument();
		expect(screen.getByText("Raise to 75%")).toBeInTheDocument();
		expect(
			screen.getByRole("slider", { name: "Page 2 playback 3 fader" }),
		).toHaveAttribute(
			"aria-description",
			"Physical 25%. Target 75%. Raise to 75%.",
		);

		rendered.rerender(
			<HardwarePlaybackCardView
				model={{
					...model,
					faderValue: 75,
					faderDisplay: "75%",
					hardwarePickup: {
						physicalPosition: 0.75,
						pickupTarget: 0.25,
					},
				}}
			/>,
		);
		expect(document.querySelector(".hardware-fader")).toHaveAttribute(
			"data-pickup-direction",
			"lower",
		);
		expect(screen.getByText("Lower to 25%")).toBeInTheDocument();
	});

	it("clamps pickup geometry and suppresses a satisfied difference", () => {
		expect(
			hardwarePickupGeometry({
				physicalPosition: -0.5,
				pickupTarget: 1.5,
			}),
		).toEqual({
			physicalPercent: 0,
			targetPercent: 100,
			segmentStartPercent: 0,
			segmentSizePercent: 100,
			direction: "raise",
		});
		expect(
			hardwarePickupGeometry({
				physicalPosition: 0.5,
				pickupTarget: 0.5,
			}),
		).toMatchObject({ segmentSizePercent: 0, direction: "satisfied" });

		render(
			<HardwarePlaybackCardView
				model={{
					...model,
					faderValue: 50,
					faderDisplay: "50%",
					hardwarePickup: {
						physicalPosition: 0.5,
						pickupTarget: 0.5,
					},
				}}
			/>,
		);
		expect(
			document.querySelector(".hardware-fader-pickup-difference"),
		).not.toBeInTheDocument();
		expect(screen.queryByText(/Physical/)).not.toBeInTheDocument();
	});

	it("composes deterministic touch and hardware banks from card view models", () => {
		const second = { ...model, slot: 4, name: "Group Master" };
		const rendered = render(
			<PlaybackBankView mode="touch" items={[{ model }, { model: second }]} />,
		);
		expect(
			document.querySelector('[data-playback-bank-mode="touch"]')?.children,
		).toHaveLength(2);

		rendered.rerender(
			<PlaybackBankView
				mode="hardware"
				items={[
					{ model, group: { name: "Front Wash", master: "62%" } },
					{ model: second },
				]}
			/>,
		);
		expect(document.querySelector(".hardware-connected")).toBeInTheDocument();
		expect(
			document.querySelector('[data-playback-bank-mode="hardware"]')?.children,
		).toHaveLength(2);
		expect(screen.getByText("Front Wash")).toBeInTheDocument();
	});

	it("keeps a faderless playback identity separate from its held action", () => {
		const press = vi.fn();
		const release = vi.fn();
		render(
			<TouchPlaybackCardView
				model={{
					...model,
					name: "Bump",
					hasFader: false,
					hardwarePickup: {
						physicalPosition: 0.8,
						pickupTarget: 0,
					},
					actions: [
						{
							id: "flash",
							label: "FLASH",
							onPointerDown: press,
							onPointerUp: release,
						},
					],
				}}
			/>,
		);
		expect(screen.getByText("3 · Bump")).toBeInTheDocument();
		const flash = screen.getByRole("button", { name: "FLASH" });
		fireEvent.pointerDown(flash, { pointerId: 1 });
		fireEvent.pointerUp(flash, { pointerId: 1 });
		expect(press).toHaveBeenCalledOnce();
		expect(release).toHaveBeenCalledOnce();
		expect(
			document.querySelector(".hardware-fader-pickup-difference"),
		).not.toBeInTheDocument();
	});

	it("never renders hardware pickup presentation in touch mode", () => {
		render(
			<TouchPlaybackCardView
				model={{
					...model,
					hardwarePickup: {
						physicalPosition: 0.8,
						pickupTarget: 0,
					},
				}}
			/>,
		);
		expect(
			document.querySelector(".hardware-fader-pickup-difference"),
		).not.toBeInTheDocument();
		expect(screen.queryByText(/Physical/)).not.toBeInTheDocument();
		expect(screen.queryByText(/Target/)).not.toBeInTheDocument();
	});

	it("renders card-level runtime statuses with explicit text", () => {
		const rendered = render(
			<TouchPlaybackCardView
				model={{
					...model,
					status: { kind: "loaded", label: "LOADED" },
				}}
			/>,
		);
		expect(screen.getByRole("status")).toHaveTextContent("LOADED");
		rendered.rerender(
			<TouchPlaybackCardView
				model={{
					...model,
					status: { kind: "flash", label: "FLASH HELD" },
				}}
			/>,
		);
		expect(screen.getByRole("status")).toHaveTextContent("FLASH HELD");
		rendered.rerender(
			<TouchPlaybackCardView
				model={{
					...model,
					status: { kind: "swap", label: "SWAP HELD" },
				}}
			/>,
		);
		expect(screen.getByRole("status")).toHaveTextContent("SWAP HELD");
	});
});
