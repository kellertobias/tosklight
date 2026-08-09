import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { OutputRoute, VersionedObject } from "../../api/types";
import { OutputRoutesSetup } from "./OutputRoutesSetup";

afterEach(cleanup);

const route: VersionedObject<OutputRoute> = {
	kind: "route",
	id: "front-artnet",
	revision: 4,
	updated_at: "2026-07-16T12:00:00Z",
	body: {
		protocol: "art_net",
		logical_universe: 1,
		destination_universe: 11,
		delivery_mode: "unicast",
		destination: "10.0.0.20:6454",
		enabled: true,
		minimum_slots: 128,
	},
};

describe("OutputRoutesSetup", () => {
	it("edits an existing versioned route without writing before Save", async () => {
		const save = vi.fn().mockResolvedValue(true);
		render(
			<OutputRoutesSetup
				routes={[route]}
				onSave={save}
				onCreateRange={vi.fn().mockResolvedValue(true)}
				onDelete={vi.fn().mockResolvedValue(true)}
			/>,
		);

		expect(screen.getByText("Logical 1 → Art-Net 11")).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Edit route" }));
		fireEvent.change(screen.getByLabelText("Logical universe"), {
			target: { value: "2" },
		});
		fireEvent.change(screen.getByLabelText("Minimum universe size"), {
			target: { value: "256" },
		});
		expect(save).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Save route" }));

		await waitFor(() =>
			expect(save).toHaveBeenCalledWith(
				"front-artnet",
				{
					...route.body,
					logical_universe: 2,
					minimum_slots: 256,
				},
				4,
			),
		);
		expect(
			screen.queryByRole("dialog", { name: "Output route editor" }),
		).not.toBeInTheDocument();
	});

	it("offers protocol-correct delivery modes and validates Unicast destinations", async () => {
		const save = vi.fn().mockResolvedValue(true);
		render(
			<OutputRoutesSetup
				routes={[]}
				onSave={save}
				onCreateRange={vi.fn().mockResolvedValue(true)}
				onDelete={vi.fn().mockResolvedValue(true)}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Add route" }));
		expect(screen.getByRole("button", { name: "Broadcast" })).toBeVisible();
		expect(screen.queryByLabelText("Destination")).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Save route" }));
		await waitFor(() =>
			expect(save).toHaveBeenCalledWith(
				expect.stringMatching(/^route-/),
				expect.objectContaining({
					delivery_mode: "broadcast",
					destination: null,
				}),
				0,
			),
		);

		fireEvent.click(screen.getByRole("button", { name: "Add route" }));
		fireEvent.click(screen.getByRole("button", { name: "Broadcast" }));
		fireEvent.click(screen.getByRole("option", { name: "Unicast" }));
		fireEvent.click(screen.getByRole("button", { name: "Save route" }));
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Unicast delivery requires an IPv4 destination and port",
		);
		fireEvent.change(screen.getByLabelText("Destination"), {
			target: { value: "127.0.0.1:6454" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save route" }));
		await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
		expect(save.mock.calls[1][1]).toMatchObject({
			delivery_mode: "unicast",
			destination: "127.0.0.1:6454",
			enabled: true,
			minimum_slots: 128,
		});

		fireEvent.click(screen.getByRole("button", { name: "Add route" }));
		fireEvent.click(screen.getByRole("button", { name: "Art-Net" }));
		fireEvent.click(screen.getByRole("option", { name: "sACN" }));
		expect(screen.getByRole("button", { name: "Multicast" })).toBeVisible();
		expect(
			screen.getByText(/derives its 239\.255\.x\.y:5568 destination/),
		).toBeVisible();
	});

	it("requires an explicit confirmation before removing a route", async () => {
		const remove = vi.fn().mockResolvedValue(true);
		render(
			<OutputRoutesSetup
				routes={[route]}
				onSave={vi.fn().mockResolvedValue(true)}
				onCreateRange={vi.fn().mockResolvedValue(true)}
				onDelete={remove}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Edit route" }));
		fireEvent.click(screen.getByRole("button", { name: "Remove route" }));
		expect(remove).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Confirm remove" }));
		await waitFor(() => expect(remove).toHaveBeenCalledWith("front-artnet", 4));
	});

	it("assigns one logical universe to a claimed USB DMX endpoint", async () => {
		const save = vi.fn().mockResolvedValue(true);
		render(
			<OutputRoutesSetup
				routes={[]}
				usbEndpoints={[
					{
						endpoint_id: "front-usb",
						driver: "enttec_usb_pro_v144",
						identity: {
							vendor_id: 0x403,
							product_id: 0x6001,
							usb_serial: "TL-USB-1",
						},
						enabled: true,
					},
				]}
				onSave={save}
				onCreateRange={vi.fn().mockResolvedValue(true)}
				onDelete={vi.fn().mockResolvedValue(true)}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Add route" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Network (Art-Net / sACN)" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "USB DMX endpoint" }));
		expect(screen.getByLabelText("Logical universe")).toBeVisible();
		expect(
			screen.queryByLabelText("Destination universe"),
		).not.toBeInTheDocument();
		fireEvent.change(screen.getByLabelText("Logical universe"), {
			target: { value: "7" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save route" }));

		await waitFor(() =>
			expect(save).toHaveBeenCalledWith(
				expect.stringMatching(/^route-/),
				expect.objectContaining({
					target: { kind: "usb_endpoint", endpoint_id: "front-usb" },
					logical_universe: 7,
					destination: null,
				}),
				0,
			),
		);
	});

	it("creates paired universe ranges with one atomic action", async () => {
		const save = vi.fn().mockResolvedValue(true);
		const createRange = vi.fn().mockResolvedValue(true);
		render(
			<OutputRoutesSetup
				routes={[]}
				onSave={save}
				onCreateRange={createRange}
				onDelete={vi.fn().mockResolvedValue(true)}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Add route" }));
		enterRange("Logical universe", ["1", "THRU", "8"]);
		enterRange("Destination universe", ["1", "THRU", "8"]);
		fireEvent.click(screen.getByRole("button", { name: "Save route" }));

		await waitFor(() =>
			expect(createRange).toHaveBeenCalledWith({
				logical_start: 1,
				logical_end: 8,
				destination_start: 1,
				destination_end: 8,
				route: {
					protocol: "art_net",
					delivery_mode: "broadcast",
					destination: null,
					enabled: true,
					minimum_slots: 128,
				},
			}),
		);
		expect(save).not.toHaveBeenCalled();
	});

	it("rejects unequal and descending ranges before transport", () => {
		const createRange = vi.fn().mockResolvedValue(true);
		render(
			<OutputRoutesSetup
				routes={[]}
				onSave={vi.fn().mockResolvedValue(true)}
				onCreateRange={createRange}
				onDelete={vi.fn().mockResolvedValue(true)}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Add route" }));
		enterRange("Logical universe", ["1", "THRU", "8"]);
		enterRange("Destination universe", ["1", "THRU", "7"]);
		fireEvent.click(screen.getByRole("button", { name: "Save route" }));
		expect(screen.getByRole("alert")).toHaveTextContent(
			"same number of universes",
		);
		expect(createRange).not.toHaveBeenCalled();

		enterRange("Logical universe", ["8", "THRU", "1"]);
		enterRange("Destination universe", ["8", "THRU", "1"]);
		fireEvent.click(screen.getByRole("button", { name: "Save route" }));
		expect(screen.getByRole("alert")).toHaveTextContent("ascending order");
		expect(createRange).not.toHaveBeenCalled();
	});
});

function enterRange(label: string, keys: string[]) {
	const input = screen.getByLabelText(label);
	const open = input.parentElement?.querySelector<HTMLButtonElement>(
		'[aria-label="Open number pad"]',
	);
	if (!open) throw new Error(`Missing number pad for ${label}`);
	fireEvent.click(open);
	const dialog = screen.getByRole("dialog", { name: label });
	for (const key of keys)
		fireEvent.click(within(dialog).getByRole("button", { name: key }));
	fireEvent.click(within(dialog).getByRole("button", { name: "ENTER" }));
}
