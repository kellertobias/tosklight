/** Offline examples for manual photographs. These do not discover or configure real hardware. */
import type {
	ExtensionRuntimeSnapshot,
	UsbDmxEndpointSnapshot,
} from "../../../light-desktop/src/api/client/deskManagement";
import type {
	MatterBridgeStatus,
	SessionResponse,
} from "../../../light-desktop/src/api/types";
import type { MediaServersState } from "../../../light-desktop/src/features/mediaServers/MediaServersContext";

export const documentationSession: SessionResponse = {
	session_id: "documentation-session",
	client_id: "documentation-client",
	token: "offline-example",
	desk: {
		id: "documentation-desk",
		name: "Main desk",
		columns: 8,
		rows: 1,
		buttons: 3,
		surface_id: "main",
	},
};
export const configuredExtensions: ExtensionRuntimeSnapshot = {
	extensions_directory: "Extensions",
	configuration_path: "Extensions/extensions.json",
	configuration_diagnostic: null,
	packages: [
		{
			id: "de.tosklight.hardware-simulator",
			name: "ToskLight Hardware Simulator",
			version: "1.0.0",
			directory: "Extensions/hardware-simulator",
			package_digest: "documentation-package",
			readiness: "ready",
			locally_approved_unsigned: true,
			diagnostics: [],
		},
	],
	instances: [
		{
			id: "hardware-simulator",
			extension_id: "de.tosklight.hardware-simulator",
			package_digest: "documentation-package",
			executable: "tl-hardware-simulator-extension",
			capabilities: ["control_surface"],
			state: "running",
			last_error: null,
			launches: 1,
			crashes: 0,
			protocol_errors: 0,
			inbound_drops: 0,
			outbound_drops: 0,
		},
	],
	instance_diagnostics: [],
};
export const configuredUsbDmx: UsbDmxEndpointSnapshot = {
	document: {
		revision: 1,
		endpoints: [
			{
				endpoint_id: "front-usb",
				driver: "enttec_usb_pro_v144",
				enabled: true,
				identity: {
					vendor_id: 0x403,
					product_id: 0x6001,
					manufacturer: "ENTTEC",
					product: "DMX USB PRO",
					usb_serial: "DEMO001",
					port_topology_hint: null,
				},
			},
		],
	},
	discovered_devices: [
		{
			port_name: "/dev/cu.usbserial-DEMO001",
			identity: {
				vendor_id: 0x403,
				product_id: 0x6001,
				manufacturer: "ENTTEC",
				product: "DMX USB PRO",
				usb_serial: "DEMO001",
			},
		},
	],
	diagnostics: [],
	replayed: false,
};
export const configuredMatter: MatterBridgeStatus = {
	enabled: true,
	transport: "running",
	network_running: true,
	commissionable: true,
	commissioned: false,
	commissioning_window_open: true,
	revision: 1,
	pairing: {
		manual_code: "34970112332",
		qr_code: "MT:EXAMPLE",
		discriminator: 3840,
	},
	lights: [
		{
			endpoint_id: 1,
			address: { page: 1, position: 1 },
			name: "Page 1 Playback 1: Front Fresnels",
			on: true,
			level: 180,
			kind: "color",
			color_active: true,
		},
		{
			endpoint_id: 2,
			address: { page: 1, position: 2 },
			name: "Page 1 Playback 2: Opening Sequence",
			on: true,
			level: 254,
			kind: "dimmable",
			color_active: false,
		},
	],
};
const unavailable = async (): Promise<never> => {
	throw new Error("This documentation example does not contact a Media Server");
};
export const configuredMedia: MediaServersState = {
	matter: configuredMatter,
	mediaServers: [],
	mediaPreviewUrls: {},
	discoverMediaServers: unavailable,
	updateDiscoveredMediaAddress: unavailable,
	refreshMediaPreview: async () => false,
	refreshMediaThumbnails: async () => false,
	clearMediaThumbnailCache: async () => 0,
	inspectMediaServer: unavailable,
	nativeMedia: unavailable,
	updateNativeMediaText: unavailable,
	updateNativeMediaEffect: unavailable,
	applyMediaLibrarySelection: unavailable,
	mediaThumbnail: unavailable,
};

/** Browser device fixture only; no microphone stream is opened. Restore descriptors on story exit. */
export function installDocumentationAudioInput() {
	const mediaDescriptor = Object.getOwnPropertyDescriptor(
		navigator,
		"mediaDevices",
	);
	const permissionsDescriptor = Object.getOwnPropertyDescriptor(
		navigator,
		"permissions",
	);
	const storageKey = "light.sound-to-light.device.documentation-desk";
	const previous = localStorage.getItem(storageKey);
	localStorage.setItem(storageKey, "documentation-line-input");
	Object.defineProperty(navigator, "mediaDevices", {
		configurable: true,
		value: {
			enumerateDevices: async () => [
				{
					kind: "audioinput",
					deviceId: "documentation-line-input",
					label: "USB Audio Interface · Line Input 1–2",
					groupId: "documentation-audio",
				},
			],
			getUserMedia: async () => ({ getTracks: () => [] }),
			addEventListener: () => undefined,
			removeEventListener: () => undefined,
		},
	});
	Object.defineProperty(navigator, "permissions", {
		configurable: true,
		value: { query: async () => ({ state: "granted" }) },
	});
	return () => {
		if (mediaDescriptor)
			Object.defineProperty(navigator, "mediaDevices", mediaDescriptor);
		else Reflect.deleteProperty(navigator, "mediaDevices");
		if (permissionsDescriptor)
			Object.defineProperty(navigator, "permissions", permissionsDescriptor);
		else Reflect.deleteProperty(navigator, "permissions");
		if (previous === null) localStorage.removeItem(storageKey);
		else localStorage.setItem(storageKey, previous);
	};
}
