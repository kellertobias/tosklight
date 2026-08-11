import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { UsbDmxEndpointSnapshot } from "../../api/client/deskManagement";
import { UsbDmxActionsProvider } from "../../features/usbDmx/UsbDmxActions";
import {
	internalUsbDmxDeviceId,
	preferredUsbDmxDevices,
	recommendedUsbDmxDriver,
	usbDmxDeviceLabel,
	useUsbDmxDiscovery,
} from "./UsbDmxEndpointsSetup";

type Device = UsbDmxEndpointSnapshot["discovered_devices"][number];

afterEach(cleanup);

function device(
	port_name: string,
	product: string,
	serial = "TL1",
	manufacturer = "ENTTEC",
): Device {
	return {
		port_name,
		identity: {
			vendor_id: 0x403,
			product_id: 0x6001,
			manufacturer,
			product,
			usb_serial: serial,
			port_topology_hint: port_name,
		},
	};
}

describe("USB DMX discovery presentation", () => {
	it("prefers the macOS callout device over its paired TTY entry", () => {
		const preferred = preferredUsbDmxDevices([
			device("/dev/tty.usbserial-TL1", "DMX USB Pro"),
			device("/dev/cu.usbserial-TL1", "DMX USB Pro"),
		]);

		expect(preferred).toHaveLength(1);
		expect(preferred[0].port_name).toBe("/dev/cu.usbserial-TL1");
	});

	it("uses device metadata for a confident driver recommendation", () => {
		expect(
			recommendedUsbDmxDriver(
				device("/dev/cu.usbserial-TL1", "DMX USB Pro"),
			),
		).toBe("enttec_usb_pro_v144");
		expect(
			recommendedUsbDmxDriver(
				device("/dev/cu.usbserial-TL2", "Open DMX", "TL2"),
			),
		).toBe("open_dmx");
		expect(
			recommendedUsbDmxDriver(
				device("/dev/cu.usbserial-TL3", "Generic FTDI", "TL3", "FTDI"),
			),
		).toBeNull();
	});

	it("presents the stable USB serial without exposing an internal identifier", () => {
		expect(
			usbDmxDeviceLabel(device("/dev/cu.usbserial-TL1", "DMX USB Pro")),
		).toBe("DMX USB Pro · USB serial TL1");
	});

	it("keeps identical serial strings distinct across USB vendor and product identities", () => {
		const first = device("/dev/cu.usbserial-shared", "DMX USB Pro", "shared");
		const second = {
			...first,
			identity: {
				...first.identity,
				vendor_id: 0x16c0,
				product_id: 0x05dc,
			},
		};

		expect(internalUsbDmxDeviceId(first)).toBe("usb-dmx-403-6001-shared");
		expect(internalUsbDmxDeviceId(second)).toBe("usb-dmx-16c0-5dc-shared");
		expect(internalUsbDmxDeviceId(first)).not.toBe(
			internalUsbDmxDeviceId(second),
		);
	});

	it("installs a refreshed discovery snapshot after Scan", async () => {
		const first: UsbDmxEndpointSnapshot = {
			document: { revision: 1, endpoints: [] },
			diagnostics: [],
			discovered_devices: [],
			replayed: false,
		};
		const refreshed: UsbDmxEndpointSnapshot = {
			...first,
			discovered_devices: [device("/dev/cu.usbserial-TL1", "DMX USB Pro")],
		};
		const load = vi
			.fn()
			.mockResolvedValueOnce(first)
			.mockResolvedValueOnce(refreshed);

		function Probe() {
			const discovery = useUsbDmxDiscovery();
			return (
				<>
					<span>{discovery.devices[0]?.identity.usb_serial ?? "No devices"}</span>
					<button type="button" onClick={() => void discovery.scan()}>
						Scan
					</button>
				</>
			);
		}

		render(
			<UsbDmxActionsProvider
				actions={{
					load,
					upsert: vi.fn(),
					remove: vi.fn(),
					resetMalformed: vi.fn(),
				}}
			>
				<Probe />
			</UsbDmxActionsProvider>,
		);
		await screen.findByText("No devices");
		fireEvent.click(screen.getByRole("button", { name: "Scan" }));
		await screen.findByText("TL1");
		expect(load).toHaveBeenCalledTimes(2);
	});
});
