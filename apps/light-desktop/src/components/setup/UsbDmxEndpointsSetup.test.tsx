import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { UsbDmxEndpointSnapshot } from "../../api/client/deskManagement";
import { UsbDmxActionsProvider } from "../../features/usbDmx/UsbDmxActions";
import { UsbDmxEndpointsSetup } from "./UsbDmxEndpointsSetup";

afterEach(cleanup);

const emptySnapshot: UsbDmxEndpointSnapshot = {
	document: { revision: 2, endpoints: [] },
	diagnostics: [],
	discovered_devices: [
		{
			port_name: "/dev/cu.usbserial-TL1",
			identity: {
				vendor_id: 0x403,
				product_id: 0x6001,
				manufacturer: "ENTTEC",
				product: "DMX USB Pro",
				usb_serial: "TL1",
				port_topology_hint: "/dev/cu.usbserial-TL1",
			},
		},
	],
	replayed: false,
};

describe("UsbDmxEndpointsSetup", () => {
	it("discovers and claims an exact USB identity with the selected driver", async () => {
		const upsert = vi.fn().mockResolvedValue({
			...emptySnapshot,
			document: {
				revision: 3,
				endpoints: [
					{
						endpoint_id: "dmx-usb-pro",
						driver: "enttec_usb_pro_v144",
						identity: {
							...emptySnapshot.discovered_devices[0].identity,
							port_topology_hint: null,
						},
						enabled: true,
					},
				],
			},
		});
		render(
			<UsbDmxActionsProvider
				actions={{
					load: vi.fn().mockResolvedValue(emptySnapshot),
					upsert,
					remove: vi.fn(),
					resetMalformed: vi.fn(),
				}}
			>
				<UsbDmxEndpointsSetup />
			</UsbDmxActionsProvider>,
		);

		await screen.findByText("No USB DMX endpoint is claimed.");
		fireEvent.click(
			screen.getByRole("button", { name: "Choose a USB serial device" }),
		);
		fireEvent.click(
			screen.getByRole("option", {
				name: "DMX USB Pro · /dev/cu.usbserial-TL1",
			}),
		);
		expect(screen.getByLabelText("Endpoint ID")).toHaveValue("dmx-usb-pro");
		fireEvent.click(screen.getByRole("button", { name: "Claim endpoint" }));

		await waitFor(() =>
			expect(upsert).toHaveBeenCalledWith(
				2,
				expect.objectContaining({
					endpoint_id: "dmx-usb-pro",
					driver: "enttec_usb_pro_v144",
					enabled: true,
					identity: expect.objectContaining({
						usb_serial: "TL1",
						port_topology_hint: null,
					}),
				}),
			),
		);
		expect(await screen.findByText("dmx-usb-pro")).toBeVisible();
	});
});
