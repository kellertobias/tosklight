import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PsnEdit, PsnSnapshot } from "../../api/client/psn";
import { PsnProvider } from "../../features/psn/PsnContext";
import { PsnSetup } from "./PsnSetup";

const POINT = "11111111-1111-4111-8111-111111111111";
const BINDING = "22222222-2222-4222-8222-222222222222";

function snapshot(overrides: Partial<PsnSnapshot> = {}): PsnSnapshot {
	return {
		revision: 3,
		configuration: {
			enabled: true,
			group: "236.10.10.10",
			port: 56565,
			interface: null,
			staleAfterMillis: 1000,
			calibration: {
				offsetMetres: [0, 0, 0],
				rotationDegrees: 0,
				scale: 1,
			},
			bindings: [],
			zones: [],
			...overrides.configuration,
		},
		status: {
			enabled: true,
			listeningOn: "236.10.10.10:56565",
			health: { state: "receiving" },
			systemNames: ["OpenFollow"],
			trackers: [
				{
					trackerId: 3,
					name: "Presenter",
					positionMetres: [1.5, 1.7, -2],
					ageMillis: 20,
					stale: false,
					source: "10.0.0.9:56565",
				},
			],
			placements: [],
			occupiedZoneIds: [],
			frames: 1200,
			ignoredDatagrams: 0,
			error: null,
			...overrides.status,
		},
		points: [{ fixtureId: POINT, name: "Follow point", fixtureNumber: 51 }],
		macros: [{ id: BINDING, number: 4, name: "Downstage special" }],
		...overrides,
	};
}

function mount(initial: PsnSnapshot, update = vi.fn().mockResolvedValue({})) {
	const psn = {
		snapshot: vi.fn().mockResolvedValue(initial),
		update,
	};
	render(
		<PsnProvider psn={psn}>
			<PsnSetup />
		</PsnProvider>,
	);
	return psn;
}

afterEach(cleanup);

describe("the Tracking tab", () => {
	it("says what is arriving, and from whom", async () => {
		mount(snapshot());

		expect(await screen.findByRole("status")).toHaveTextContent(
			/Receiving on 236\.10\.10\.10:56565 from OpenFollow/,
		);
		expect(screen.getByText(/1\.50m, 1\.70m, -2\.00m/)).toBeInTheDocument();
	});

	it("tells the operator a quiet source is holding rather than broken", async () => {
		// The distinction the whole feature rests on: nothing is arriving, and the lights have
		// not moved back to where the show says. That has to be readable at a glance.
		mount(
			snapshot({
				status: {
					...snapshot().status,
					health: { state: "stale", silentForMillis: 4000 },
				},
			}),
		);

		expect(await screen.findByRole("status")).toHaveTextContent(
			/Nothing heard .* for 4s\. Bound points are holding their last position\./,
		);
	});

	it("says plainly that traffic alone moves nothing", async () => {
		mount(snapshot());

		expect(
			await screen.findByText(
				/A tracker moves a 3D Point only once it has been given one here\./,
			),
		).toBeInTheDocument();
	});

	it("binds a tracker to a 3D Point the operator picks", async () => {
		const update = vi.fn().mockResolvedValue({});
		mount(snapshot(), update);
		const chooser = await screen.findByRole("button", {
			name: "3D Point for tracker 3",
		});

		// A SelectField is a listbox behind a button rather than a native select.
		await userEvent.click(chooser);
		await userEvent.click(
			await screen.findByRole("option", { name: "51 · Follow point" }),
		);

		await waitFor(() => expect(update).toHaveBeenCalled());
		const edit = update.mock.calls[0][0] as PsnEdit;
		expect(edit.bindings).toEqual([
			expect.objectContaining({ trackerId: 3, pointFixtureId: POINT }),
		]);
	});

	it("turns the source off without forgetting the bindings", async () => {
		const update = vi.fn().mockResolvedValue({});
		const initial = snapshot();
		initial.configuration.bindings = [
			{ id: BINDING, trackerId: 3, pointFixtureId: POINT, enabled: true },
		];
		mount(initial, update);

		await userEvent.click(
			await screen.findByRole("switch", { name: /Receive PosiStageNet/ }),
		);

		await waitFor(() => expect(update).toHaveBeenCalled());
		// Only what changed travels, so the desk keeps which tracker was which point.
		expect(update.mock.calls[0][0]).toEqual({ enabled: false });
	});

	it("shows where a bound point ended up, and when it could not reach", async () => {
		const initial = snapshot();
		initial.configuration.bindings = [
			{ id: BINDING, trackerId: 3, pointFixtureId: POINT, enabled: true },
		];
		initial.status.placements = [
			{
				bindingId: BINDING,
				pointFixtureId: POINT,
				positionMetres: [100, 0, 0],
				outOfReach: true,
			},
		];
		mount(initial);

		expect(await screen.findByText(/out of reach/)).toBeInTheDocument();
	});
});
