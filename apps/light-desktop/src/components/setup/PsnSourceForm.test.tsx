import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PsnConfiguration, PsnSnapshot } from "../../api/client/psn";
import { PsnProvider } from "../../features/psn/PsnContext";
import { TrackingSettingsForm, validateTrackingSource } from "./PsnSourceForm";

function configuration(
	overrides: Partial<PsnConfiguration> = {},
): PsnConfiguration {
	return {
		enabled: true,
		group: "236.10.10.10",
		port: 56565,
		interface: null,
		staleAfterMillis: 1000,
		calibration: { offsetMetres: [0, 0, 0], rotationDegrees: 0, scale: 1 },
		bindings: [],
		zones: [],
		...overrides,
	};
}

function snapshot(config: PsnConfiguration): PsnSnapshot {
	return {
		revision: 1,
		configuration: config,
		status: {
			enabled: true,
			listeningOn: null,
			health: null,
			systemNames: [],
			trackers: [],
			placements: [],
			occupiedZoneIds: [],
			frames: 0,
			ignoredDatagrams: 0,
			error: null,
		},
		points: [],
		macros: [],
	};
}

/** A desk double that stores what it accepts, so a reopen reads the saved values back. */
function desk(refuse?: string) {
	let stored = configuration();
	const psn = {
		snapshot: vi.fn(async () => snapshot(stored)),
		update: vi.fn(async (edit: Partial<PsnConfiguration>) => {
			if (refuse) throw new Error(refuse);
			stored = { ...stored, ...edit };
			return stored;
		}),
	};
	const mount = () =>
		render(
			<PsnProvider psn={psn}>
				<TrackingSettingsForm />
			</PsnProvider>,
		);
	return { psn, mount };
}

afterEach(cleanup);

describe("Tracking Settings", () => {
	it("holds the multicast group, port and Stale after with the current values", async () => {
		desk().mount();
		expect(await screen.findByLabelText("Multicast group")).toHaveValue(
			"236.10.10.10",
		);
		expect(screen.getByLabelText("Port")).toHaveValue("56565");
		expect(screen.getByLabelText("Stale after (ms)")).toHaveValue("1000");
		expect(
			screen.getByRole("button", { name: "Apply tracking settings" }),
		).toBeDisabled();
	});

	it("applies the changed values together and reads them back after a reopen", async () => {
		const { psn, mount } = desk();
		const first = mount();
		fireEvent.change(await screen.findByLabelText("Multicast group"), {
			target: { value: "239.1.2.3" },
		});
		fireEvent.change(screen.getByLabelText("Port"), {
			target: { value: "56570" },
		});
		fireEvent.change(screen.getByLabelText("Stale after (ms)"), {
			target: { value: "2500" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Apply tracking settings" }),
		);
		expect(await screen.findByRole("status")).toHaveTextContent(
			"Tracking settings saved.",
		);
		expect(psn.update).toHaveBeenCalledWith({
			group: "239.1.2.3",
			port: 56570,
			staleAfterMillis: 2500,
		});
		first.unmount();

		mount();
		expect(await screen.findByLabelText("Multicast group")).toHaveValue(
			"239.1.2.3",
		);
		expect(screen.getByLabelText("Port")).toHaveValue("56570");
		expect(screen.getByLabelText("Stale after (ms)")).toHaveValue("2500");
	});

	it("names what is wrong beside the field and sends nothing", async () => {
		const { psn, mount } = desk();
		mount();
		fireEvent.change(await screen.findByLabelText("Multicast group"), {
			target: { value: "10.0.0.1" },
		});
		fireEvent.change(screen.getByLabelText("Port"), {
			target: { value: "0" },
		});
		fireEvent.change(screen.getByLabelText("Stale after (ms)"), {
			target: { value: "10" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Apply tracking settings" }),
		);
		expect(
			await screen.findByText(/use 224\.0\.0\.0 to 239\.255\.255\.255/),
		).toBeInTheDocument();
		expect(
			screen.getByText("Enter a port from 1 to 65535."),
		).toBeInTheDocument();
		expect(
			screen.getByText("Enter a whole number from 50 to 60000 milliseconds."),
		).toBeInTheDocument();
		expect(psn.update).not.toHaveBeenCalled();
	});

	it("shows the desk's refusal as an actionable error", async () => {
		const { mount } = desk("the PSN port must not be 0");
		mount();
		fireEvent.change(await screen.findByLabelText("Port"), {
			target: { value: "56571" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Apply tracking settings" }),
		);
		expect(await screen.findByRole("alert")).toHaveTextContent(
			/refused the tracking settings: the PSN port must not be 0\. Correct the value and apply again\./,
		);
	});

	it("validates the same limits the desk enforces", () => {
		expect(
			validateTrackingSource({
				group: "236.10.10.10",
				port: "56565",
				staleAfter: "50",
			}),
		).toEqual({});
		expect(
			Object.keys(
				validateTrackingSource({
					group: "236.10.10",
					port: "65536",
					staleAfter: "60001",
				}),
			),
		).toEqual(["group", "port", "staleAfter"]);
	});
});
