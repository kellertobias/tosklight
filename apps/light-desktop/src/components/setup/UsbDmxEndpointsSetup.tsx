import { useCallback, useEffect, useState } from "react";
import type {
	UsbDmxDriverKind,
	UsbDmxEndpoint,
	UsbDmxEndpointSnapshot,
} from "../../api/client/deskManagement";
import { useUsbDmxActions } from "../../features/usbDmx/UsbDmxActions";

export type DiscoveredUsbDmxDevice =
	UsbDmxEndpointSnapshot["discovered_devices"][number];

function deviceIdentityKey(device: DiscoveredUsbDmxDevice) {
	const { identity } = device;
	return identity.usb_serial
		? `${identity.vendor_id}:${identity.product_id}:serial:${identity.usb_serial}`
		: `${identity.vendor_id}:${identity.product_id}:port:${device.port_name.replace(/^\/dev\/(?:cu|tty)\./u, "")}`;
}

/** Prefer the macOS callout device and hide its paired dial-in entry. */
export function preferredUsbDmxDevices(
	devices: readonly DiscoveredUsbDmxDevice[],
): DiscoveredUsbDmxDevice[] {
	const preferred = new Map<string, DiscoveredUsbDmxDevice>();
	for (const device of devices) {
		const key = deviceIdentityKey(device);
		const current = preferred.get(key);
		if (
			!current ||
			(device.port_name.startsWith("/dev/cu.") &&
				current.port_name.startsWith("/dev/tty."))
		)
			preferred.set(key, device);
	}
	return [...preferred.values()].sort((left, right) =>
		left.port_name.localeCompare(right.port_name),
	);
}

export function recommendedUsbDmxDriver(
	device: DiscoveredUsbDmxDevice,
): UsbDmxDriverKind | null {
	const description = [device.identity.manufacturer, device.identity.product]
		.filter(Boolean)
		.join(" ");
	if (/open\s*dmx/iu.test(description)) return "open_dmx";
	if (/enttec|dmx\s*usb\s*pro|usb\s*pro/iu.test(description))
		return "enttec_usb_pro_v144";
	return null;
}

export function usbDmxDeviceLabel(
	device: Pick<DiscoveredUsbDmxDevice, "identity" | "port_name">,
) {
	const product = device.identity.product?.trim() || "USB DMX device";
	const serial = device.identity.usb_serial?.trim();
	return serial
		? `${product} · USB serial ${serial}`
		: `${product} · ${device.port_name}`;
}

export function endpointForDevice(
	endpoints: readonly UsbDmxEndpoint[],
	device: DiscoveredUsbDmxDevice,
) {
	return endpoints.find((endpoint) => {
		if (
			endpoint.identity.vendor_id !== device.identity.vendor_id ||
			endpoint.identity.product_id !== device.identity.product_id
		)
			return false;
		return device.identity.usb_serial
			? endpoint.identity.usb_serial === device.identity.usb_serial
			: endpoint.identity.port_topology_hint === device.port_name;
	});
}

export function internalUsbDmxDeviceId(device: DiscoveredUsbDmxDevice) {
	const serialOrPort =
		device.identity.usb_serial || device.port_name.replace(/^\/dev\//u, "");
	const source = `${device.identity.vendor_id.toString(16)}-${device.identity.product_id.toString(16)}-${serialOrPort}`;
	const slug = source
		.toLowerCase()
		.replace(/[^a-z0-9]+/gu, "-")
		.replace(/^-|-$/gu, "");
	return `usb-dmx-${slug || crypto.randomUUID()}`;
}

export function useUsbDmxDiscovery() {
	const actions = useUsbDmxActions();
	const [snapshot, setSnapshot] = useState<UsbDmxEndpointSnapshot | null>(null);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState("");
	const scan = useCallback(async (): Promise<boolean> => {
		if (!actions) return false;
		setBusy(true);
		setError("");
		try {
			setSnapshot(await actions.load());
			return true;
		} catch {
			setError(
				"USB DMX devices are unavailable. Check the connection and scan again.",
			);
			return false;
		} finally {
			setBusy(false);
		}
	}, [actions]);
	useEffect(() => {
		void scan();
	}, [scan]);
	const provision = useCallback(
		async (device: DiscoveredUsbDmxDevice, driver: UsbDmxDriverKind) => {
			if (!actions || !snapshot) return null;
			const existing = endpointForDevice(snapshot.document.endpoints, device);
			if (existing) return existing;
			const endpoint: UsbDmxEndpoint = {
				endpoint_id: internalUsbDmxDeviceId(device),
				driver,
				identity: {
					...device.identity,
					port_topology_hint: device.identity.usb_serial
						? null
						: device.port_name,
				},
				enabled: true,
			};
			setBusy(true);
			setError("");
			try {
				const next = await actions.upsert(snapshot.document.revision, endpoint);
				setSnapshot(next);
				return endpointForDevice(next.document.endpoints, device) ?? endpoint;
			} catch {
				setError(
					"The USB DMX device could not be prepared. Scan again and retry.",
				);
				return null;
			} finally {
				setBusy(false);
			}
		},
		[actions, snapshot],
	);
	return {
		snapshot,
		devices: preferredUsbDmxDevices(snapshot?.discovered_devices ?? []),
		busy,
		error:
			error ||
			snapshot?.discovery_error ||
			(snapshot?.configuration_error
				? "Stored USB DMX device settings need repair before a route can be added."
				: ""),
		scan,
		provision,
	};
}
