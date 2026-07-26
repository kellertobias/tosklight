import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	type VirtualPlaybackBoxViewModel,
	type VirtualPlaybackGridCallbacks,
	VirtualPlaybackGridView,
} from "./VirtualPlaybackGrid";

afterEach(cleanup);

const boxes: VirtualPlaybackBoxViewModel[] = [
	{
		slot: 1,
		position: 0,
		availability: "assigned",
		label: "Main",
		actionLabel: "GO",
		currentCue: "Cue 4",
		running: true,
	},
	{
		slot: 3,
		position: 2,
		availability: "unavailable",
	},
];

function renderGrid(
	input: {
		page?: number;
		boxes?: VirtualPlaybackBoxViewModel[];
		callbacks?: VirtualPlaybackGridCallbacks;
	} = {},
) {
	return render(
		<div style={{ width: 600, height: 500 }}>
			<VirtualPlaybackGridView
				page={input.page ?? 1}
				rows={2}
				columns={2}
				boxes={input.boxes ?? boxes}
				callbacks={input.callbacks}
			/>
		</div>,
	);
}

describe("VirtualPlaybackGridView", () => {
	it("uses one vacant design for empty and unavailable boxes without faders", () => {
		renderGrid();
		const grid = document.querySelector(".virtual-playback-grid");
		expect(grid).toHaveClass("compact-grid");
		expect(grid).toHaveStyle({
			gridTemplateColumns:
				"repeat(2, minmax(var(--grid-cell-min), 1fr))",
			gridTemplateRows: "repeat(2, minmax(0, 1fr))",
		});
		expect(document.querySelectorAll(".virtual-playback-box")).toHaveLength(4);
		expect(document.querySelector('[data-grid-position="0"]')).toHaveClass(
			"playback-colored",
			"running",
		);
		expect(document.querySelector('[data-grid-position="1"]')).toHaveClass(
			"vacant",
		);
		expect(document.querySelector('[data-grid-position="2"]')).toHaveClass(
			"vacant",
		);
		expect(document.querySelector('[data-grid-position="1"]')).toHaveAttribute(
			"data-availability",
			"empty",
		);
		expect(document.querySelector('[data-grid-position="2"]')).toHaveAttribute(
			"data-availability",
			"unavailable",
		);
		expect(
			screen.getByRole("button", { name: /cell 2 empty/u }),
		).toBeEnabled();
		expect(
			screen.getByRole("button", { name: /cell 3 empty/u }),
		).toBeDisabled();
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
		expect(document.querySelector(".pool-card-status")).toHaveTextContent(
			"Running",
		);
		expect(document.querySelector(".virtual-playback-box em")).toBeNull();
	});

	it("preserves grid positions while a page changes its assignments", () => {
		const rendered = renderGrid();
		const position = () =>
			document.querySelector('[data-grid-position="0"]') as HTMLElement;
		expect(position()).toHaveAttribute("data-virtual-playback-slot", "1");
		expect(position()).toHaveTextContent("Main");

		rendered.rerender(
			<div style={{ width: 600, height: 500 }}>
				<VirtualPlaybackGridView
					page={2}
					rows={2}
					columns={2}
					boxes={[
						{
							slot: 1,
							position: 0,
							availability: "assigned",
							label: "House",
							actionLabel: "TOGGLE",
						},
					]}
				/>
			</div>,
		);
		expect(position()).toHaveAttribute("data-grid-position", "0");
		expect(position()).toHaveAttribute("data-page", "2");
		expect(position()).toHaveTextContent("House");
	});

	it("executes a GO-like action once and matches held press and release", () => {
		const action = vi.fn();
		const press = vi.fn();
		const release = vi.fn();
		renderGrid({
			boxes: [
				boxes[0],
				{
					slot: 2,
					position: 1,
					availability: "assigned",
					label: "Bump",
					actionLabel: "FLASH",
					heldAction: true,
				},
			],
			callbacks: {
				onAction: action,
				onActionPress: press,
				onActionRelease: release,
			},
		});

		fireEvent.click(screen.getByRole("button", { name: /cell 1 Main/ }));
		const flash = screen.getByRole("button", { name: /cell 2 Bump/ });
		fireEvent.pointerDown(flash, { pointerId: 2 });
		expect(flash).toHaveClass("held-active", "active");
		expect(flash.querySelector(".pool-card-status")).toHaveTextContent(
			"FLASH held",
		);
		fireEvent.pointerUp(flash, { pointerId: 2 });
		expect(flash).not.toHaveClass("held-active");
		fireEvent.click(flash);

		expect(action).toHaveBeenCalledOnce();
		expect(action).toHaveBeenCalledWith(1, 0);
		expect(press).toHaveBeenCalledWith(2, 1);
		expect(release).toHaveBeenCalledWith(2, 1);
	});

	it("routes configuration, assignment, update, and exclusion selection states", () => {
		const callbacks = {
			onConfigure: vi.fn(),
			onAssign: vi.fn(),
			onUpdate: vi.fn(),
			onZoneSelection: vi.fn(),
		};
		renderGrid({
			boxes: [
				{
					slot: 1,
					position: 0,
					availability: "assigned",
					label: "Configure",
					configurationTarget: true,
				},
				{ slot: 2, position: 1, availability: "empty", assignmentTarget: true },
				{
					slot: 3,
					position: 2,
					availability: "assigned",
					label: "Update",
					updateTarget: true,
				},
				{
					slot: 4,
					position: 3,
					availability: "assigned",
					label: "Zone",
					exclusionMember: true,
					exclusionSelected: true,
					selectingExclusionZone: true,
				},
			],
			callbacks,
		});

		for (const name of ["Configure", "empty", "Update", "Zone"])
			fireEvent.click(
				screen.getByRole("button", { name: new RegExp(name, "i") }),
			);

		expect(callbacks.onConfigure).toHaveBeenCalledWith(1, 0);
		expect(callbacks.onAssign).toHaveBeenCalledWith(2, 1);
		expect(callbacks.onUpdate).toHaveBeenCalledWith(3, 2);
		expect(callbacks.onZoneSelection).toHaveBeenCalledWith(4, 3);
		expect(document.querySelector('[data-grid-position="3"]')).toHaveClass(
			"exclusion-member",
			"exclusion-selected",
		);
		expect(
			document.querySelector('[data-grid-position="0"] .pool-card-workflow'),
		).toHaveTextContent("Configure Playback");
		expect(
			document.querySelector('[data-grid-position="1"] .pool-card-workflow'),
		).toHaveTextContent("Record");
		expect(
			document.querySelector('[data-grid-position="2"] .pool-card-workflow'),
		).toHaveTextContent("Update");
	});

	it("uses bottom-right icon or image artwork and keeps color contrast metadata", () => {
		renderGrid({
			boxes: [
				{
					slot: 1,
					position: 0,
					availability: "assigned",
					label: "Icon",
					icon: "☀",
					color: "#111827",
				},
				{
					slot: 2,
					position: 1,
					availability: "assigned",
					label: "Image",
					icon: "hidden",
					backgroundImage: "data:image/svg+xml,%3Csvg/%3E",
					color: "#f6d365",
					running: true,
				},
			],
		});
		expect(
			document.querySelector('[data-grid-position="0"] .pool-card-icon'),
		).toHaveTextContent("☀");
		const image = document.querySelector(
			'[data-grid-position="1"] .pool-card-image',
		);
		expect(image).toHaveAttribute("alt", "Image artwork");
		expect(
			document.querySelector('[data-grid-position="1"] .pool-card-icon'),
		).toBeNull();
		expect(document.querySelector('[data-grid-position="0"]')).toHaveStyle({
			"--playback-contrast": "#ffffff",
		});
		expect(document.querySelector('[data-grid-position="1"]')).toHaveStyle({
			"--playback-contrast": "#071014",
		});
	});
});
