import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { SpeedGroupReceptionView } from "../../shared/api/generated/media-wire";
import { SpeedGroupReception } from "./SpeedGroupStatus";

function aReception(
	overrides: Partial<SpeedGroupReceptionView> = {},
): SpeedGroupReceptionView {
	return {
		connection: "connected",
		listening: "0.0.0.0:4810",
		detail: null,
		sender: "tosklight-desk-1",
		senderAddress: "192.168.1.10:50123",
		lastUpdateAgeMillis: 120,
		accepted: 42,
		rejected: 0,
		rejections: [],
		groups: [
			{
				group: 1,
				bpm: 128,
				beatPhase: 0.25,
				running: true,
				fresh: true,
				ageMillis: 120,
			},
			{
				group: 2,
				bpm: 90,
				beatPhase: 0,
				running: false,
				fresh: true,
				ageMillis: 120,
			},
		],
		...overrides,
	};
}

describe("Speed Group reception", () => {
	it("shows the followed desk and what each group last said", () => {
		render(<SpeedGroupReception reception={aReception()} connected />);

		expect(screen.getByRole("status")).toHaveTextContent(
			"Receiving from a Light desk.",
		);
		expect(
			screen.getByText("tosklight-desk-1 (192.168.1.10:50123)"),
		).toBeVisible();
		expect(screen.getByText("42 accepted, 0 refused")).toBeVisible();
		const table = screen.getByRole("table", { name: "Received Speed Groups" });
		const rows = within(table).getAllByRole("row");
		expect(rows[1]).toHaveTextContent("1128.0Running");
		expect(rows[2]).toHaveTextContent("290.0Paused");
	});

	it("warns when the desk is lost and every group holds its tempo", () => {
		render(
			<SpeedGroupReception
				reception={aReception({
					connection: "lost",
					lastUpdateAgeMillis: 4_200,
					groups: [
						{
							group: 1,
							bpm: 128,
							beatPhase: 0,
							running: true,
							fresh: false,
							ageMillis: 4_200,
						},
					],
				})}
				connected
			/>,
		);

		expect(screen.getByRole("alert")).toHaveTextContent(
			"every group holds its last tempo",
		);
		expect(screen.getByText("4.2 s ago")).toBeVisible();
		expect(screen.getByText("Stale, holding")).toBeVisible();
	});

	it("lists refused messages with their reason and sender", () => {
		render(
			<SpeedGroupReception
				reception={aReception({
					rejected: 1,
					rejections: [
						{
							from: "192.168.1.77:9000",
							reason: "invalid message: BPM 2000 is outside 0–999",
							ageMillis: 300,
						},
					],
				})}
				connected
			/>,
		);

		expect(screen.getByText("Refused messages")).toBeVisible();
		expect(
			screen.getByText(
				"192.168.1.77:9000: invalid message: BPM 2000 is outside 0–999 (300 ms ago)",
			),
		).toBeVisible();
	});

	it("says why the listener is not running", () => {
		render(
			<SpeedGroupReception
				reception={aReception({
					connection: "unavailable",
					listening: null,
					detail: "cannot bind Speed Groups to 0.0.0.0:4810: address in use.",
					sender: null,
					senderAddress: null,
					lastUpdateAgeMillis: null,
					accepted: 0,
					groups: [],
				})}
				connected
			/>,
		);

		expect(screen.getByRole("alert")).toHaveTextContent(
			"Not listening. cannot bind Speed Groups to 0.0.0.0:4810",
		);
		expect(screen.getByText("never")).toBeVisible();
		expect(screen.queryByRole("table")).not.toBeInTheDocument();
	});

	it("does not pretend to know the state before live status arrives", () => {
		render(<SpeedGroupReception reception={undefined} connected={false} />);
		expect(
			screen.getByText("Not connected to this server's live status."),
		).toBeVisible();
	});
});
