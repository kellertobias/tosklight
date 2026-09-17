import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	dmxOutputHealth,
	dmxPatchedFixtures,
	dmxSnapshot,
} from "../../../ui-library/storybook/fixtures/dmx";
import type {
	NetworkEndpoint,
	NetworkEndpointsSnapshot,
} from "../api/generated/light-wire";
import { universeRanges } from "./DmxNodesView";
import { DmxWindowView } from "./DmxWindow";

beforeEach(() => {
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
});
afterEach(cleanup);

function endpoint(overrides: Partial<NetworkEndpoint>): NetworkEndpoint {
	return {
		id: "id",
		protocol: "art_net",
		direction: "send",
		origin: "configured",
		role: "DMX output",
		endpoint: "255.255.255.255:6454",
		name: null,
		delivery_mode: null,
		logical_universe: null,
		universes: [],
		status: "active",
		detail: "",
		errors: 0,
		last_activity_millis_ago: null,
		...overrides,
	};
}

const snapshot: NetworkEndpointsSnapshot = {
	output_bind_ip: "0.0.0.0",
	network_output_available: true,
	endpoints: [
		endpoint({
			id: "send:artnet:1:4:255.255.255.255:6454",
			delivery_mode: "broadcast",
			logical_universe: 1,
			universes: [4],
			detail: "Sending logical universe 1 as Art-Net universe 4.",
			last_activity_millis_ago: 20,
		}),
		endpoint({
			id: "send:sacn:2:9:239.255.0.9:5568",
			protocol: "sacn",
			endpoint: "239.255.0.9:5568",
			delivery_mode: "multicast",
			logical_universe: 2,
			universes: [9],
			status: "conflict",
			detail:
				"sACN universe 9 is also sent by Backup console (10.0.0.50). Move one of them.",
		}),
		endpoint({
			id: "receive:artnet:poller:10.0.0.60:6454",
			direction: "receive",
			origin: "observed",
			role: "Controller polling the desk",
			endpoint: "10.0.0.60:6454",
			detail: "Sent 2 ArtPoll(s); the desk answers each.",
		}),
		endpoint({
			id: "receive:sacn:source:ab",
			protocol: "sacn",
			direction: "receive",
			origin: "observed",
			role: "sACN source",
			endpoint: "10.0.0.50:5568",
			name: "Backup console",
			universes: [9, 10, 11, 20],
			status: "conflict",
			detail: "Also sends sACN universe 9 that this desk sends.",
		}),
		endpoint({
			id: "receive:sacn:discovery",
			protocol: "sacn",
			direction: "receive",
			role: "Universe discovery listener",
			endpoint: "239.255.250.214:5568",
			status: "unavailable",
			detail: "Could not join the sACN discovery group.",
		}),
	],
};

function renderNodes(
	overrides: Partial<Parameters<typeof DmxWindowView>[0]> = {},
) {
	const onViewChange = vi.fn();
	const rendered = render(
		<DmxWindowView
			dotSize="small"
			onDotSizeChange={vi.fn()}
			onSetDmxOverride={vi.fn()}
			outputHealth={dmxOutputHealth}
			outputRoutes={[]}
			patchedFixtures={dmxPatchedFixtures}
			snapshot={dmxSnapshot}
			view="nodes"
			onViewChange={onViewChange}
			networkEndpoints={snapshot}
			networkEndpointsSupported
			{...overrides}
		/>,
	);
	return { ...rendered, onViewChange };
}

function row(id: string): HTMLElement {
	const found = document.querySelector<HTMLElement>(
		`tr[data-endpoint-id="${id}"]`,
	);
	if (!found) throw new Error(`missing row ${id}`);
	return found;
}

describe("DMX Nodes tab", () => {
	it("is a tab beside Values and Sources and reports tab changes to the window", () => {
		const { onViewChange } = renderNodes({ view: "values" });
		const tab = screen.getByRole("tab", { name: "Nodes" });
		expect(
			screen.getByRole("tab", { name: "Values" }).compareDocumentPosition(tab) &
				Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		fireEvent.click(tab);
		expect(onViewChange).toHaveBeenCalledWith("nodes");
	});

	it("lists Art-Net and sACN send and receive endpoints with protocol, direction, endpoint, universe and status", () => {
		renderNodes();
		const table = screen.getByRole("table", { name: "Network nodes" });
		expect(
			within(table)
				.getAllByRole("columnheader")
				.map((header) => header.textContent),
		).toEqual(["Protocol", "Direction", "Endpoint", "Universe", "Status"]);

		const artNetSend = within(row("send:artnet:1:4:255.255.255.255:6454"));
		expect(artNetSend.getByText("Art-Net")).toBeTruthy();
		expect(artNetSend.getByText("Broadcast")).toBeTruthy();
		expect(artNetSend.getByText("Send")).toBeTruthy();
		expect(artNetSend.getByText("255.255.255.255:6454")).toBeTruthy();
		expect(artNetSend.getByText("1 → 4")).toBeTruthy();
		expect(artNetSend.getByText("Active")).toBeTruthy();

		const sacnSend = within(row("send:sacn:2:9:239.255.0.9:5568"));
		expect(sacnSend.getByText("sACN")).toBeTruthy();
		expect(sacnSend.getByText("Multicast")).toBeTruthy();
		expect(sacnSend.getByText("Conflict")).toBeTruthy();
		expect(sacnSend.getByText(/also sent by Backup console/)).toBeTruthy();

		const artNetReceive = within(row("receive:artnet:poller:10.0.0.60:6454"));
		expect(artNetReceive.getByText("Receive")).toBeTruthy();
		expect(artNetReceive.getByText("Heard")).toBeTruthy();
		expect(artNetReceive.getByText("—")).toBeTruthy();

		const sacnReceive = within(row("receive:sacn:source:ab"));
		expect(sacnReceive.getByText("sACN source · Backup console")).toBeTruthy();
		expect(sacnReceive.getByText("9–11, 20")).toBeTruthy();

		expect(
			within(row("receive:sacn:discovery")).getByText("Unavailable"),
		).toBeTruthy();
	});

	it("summarises endpoints per protocol and direction and lists what needs attention", () => {
		renderNodes();
		const pane = document.querySelector<HTMLElement>(".dmx-nodes-pane");
		if (!pane) throw new Error("missing pane");
		const counts = within(pane);
		expect(counts.getByText("0.0.0.0 · network output running")).toBeTruthy();
		const values = Array.from(
			pane.querySelectorAll(".dmx-nodes-counts dd"),
		).map((cell) => cell.textContent);
		expect(values).toEqual(["1", "1", "1", "2"]);
		const attention = pane.querySelector(".dmx-nodes-attention");
		expect(attention?.textContent).toContain("sACN 239.255.0.9:5568");
		expect(attention?.textContent).toContain("sACN 239.255.250.214:5568");
		expect(attention?.textContent).not.toContain("255.255.255.255");
	});

	it("shows an endpoint's details and actionable status when selected", () => {
		renderNodes();
		fireEvent.click(screen.getByRole("button", { name: "10.0.0.50:5568" }));
		const pane = document.querySelector<HTMLElement>(".dmx-nodes-pane");
		if (!pane) throw new Error("missing pane");
		expect(within(pane).getByText("Selected endpoint")).toBeTruthy();
		expect(within(pane).getByText("Heard on the network")).toBeTruthy();
		expect(within(pane).getByText("Status · Conflict")).toBeTruthy();
		fireEvent.click(within(pane).getByRole("button", { name: "Deselect" }));
		expect(within(pane).getByText("Network summary")).toBeTruthy();
	});

	it("explains empty, loading, unsupported and failed reads", () => {
		const { rerender } = renderNodes({ networkEndpoints: null });
		expect(screen.getByText("Reading network state…")).toBeTruthy();

		const props = {
			dotSize: "small" as const,
			onDotSizeChange: vi.fn(),
			onSetDmxOverride: vi.fn(),
			outputHealth: dmxOutputHealth,
			outputRoutes: [],
			patchedFixtures: dmxPatchedFixtures,
			snapshot: dmxSnapshot,
			view: "nodes" as const,
		};
		rerender(
			<DmxWindowView
				{...props}
				networkEndpointsSupported
				networkEndpoints={{ ...snapshot, endpoints: [] }}
			/>,
		);
		expect(
			screen.getByText(/No Art-Net or sACN endpoint\. Add an output route/),
		).toBeTruthy();

		rerender(<DmxWindowView {...props} />);
		expect(
			screen.getByText(/available when the desk is connected to its server/),
		).toBeTruthy();

		rerender(
			<DmxWindowView
				{...props}
				networkEndpointsSupported
				networkEndpoints={snapshot}
				networkEndpointsError="HTTP 503"
			/>,
		);
		const alert = screen.getByRole("alert");
		expect(alert.textContent).toContain("Network state could not be read.");
		expect(alert.textContent).toContain("HTTP 503");
		// The last known state stays on screen beside the error.
		expect(screen.getByRole("table", { name: "Network nodes" })).toBeTruthy();
	});

	it("compresses universes into ranges", () => {
		expect(universeRanges([7, 1, 2, 3, 4, 4])).toBe("1–4, 7");
		expect(universeRanges([5])).toBe("5");
		expect(universeRanges([])).toBe("");
	});
});
