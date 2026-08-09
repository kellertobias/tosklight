export type DummyMediaPreviewState =
	| "ready"
	| "offline"
	| "stale"
	| "failed"
	| "missing-patch"
	| "unsupported";

export interface DummyMediaAsset {
	id: string;
	name: string;
	thumbnail: string;
	detail: string;
}

export interface DummyMediaFolder {
	id: string;
	name: string;
	assets: DummyMediaAsset[];
}

export interface DummyMediaLayer {
	id: string;
	name: string;
	status: DummyMediaPreviewState;
	statusDetail: string;
	preview: string;
	liveFolderId: string;
	liveFileId: string;
	maskFolderId: string | null;
	maskFileId: string | null;
}

export interface DummyMediaServer {
	id: string;
	name: string;
	address: string;
	status: DummyMediaPreviewState;
	statusDetail: string;
	programPreview: string;
	layers: DummyMediaLayer[];
	supportsMasks: boolean;
	supportsAudio: boolean;
	supportsEffects: boolean;
}

function previewSvg(
	label: string,
	from: string,
	to: string,
	accent: string,
): string {
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 360" role="img" aria-label="${label}"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs><rect width="640" height="360" fill="url(#g)"/><circle cx="472" cy="128" r="82" fill="${accent}" fill-opacity=".72"/><path d="M0 294L132 188l98 70 112-126 106 96 78-62 114 91v103H0z" fill="#07101f" fill-opacity=".72"/><text x="32" y="52" fill="white" font-family="system-ui,sans-serif" font-size="24" font-weight="700">${label}</text></svg>`;
	return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

const previews = {
	program: previewSvg("Aurora program", "#0a223d", "#7a215d", "#ffb44a"),
	city: previewSvg("City Loop", "#12395d", "#14233a", "#77c8ff"),
	particles: previewSvg("Gold Particles", "#432c10", "#171111", "#ffd269"),
	titles: previewSvg("Tour Titles", "#401a44", "#19182f", "#ef7aff"),
	forest: previewSvg("Night Forest", "#092d2a", "#111827", "#54d68b"),
	mask: previewSvg("Soft Circle Mask", "#171717", "#030303", "#f4f4f5"),
	checker: previewSvg("Checker Mask", "#262626", "#050505", "#a3a3a3"),
} as const;

export const dummyMediaFolders: DummyMediaFolder[] = [
	{
		id: "folder-city",
		name: "City",
		assets: [
			{
				id: "file-city-loop",
				name: "City Loop",
				thumbnail: previews.city,
				detail: "00:24 · 1920×1080",
			},
			{
				id: "file-gold-particles",
				name: "Gold Particles",
				thumbnail: previews.particles,
				detail: "00:18 · 1920×1080",
			},
			...Array.from({ length: 10 }, (_, index) => ({
				id: `file-city-extra-${index + 1}`,
				name: [
					"Neon Street",
					"Blue Skyline",
					"Traffic Trails",
					"Glass Towers",
					"Night Drive",
					"Metro Map",
					"Rain Window",
					"Tunnel Run",
					"City Grid",
					"Dawn Roofs",
				][index],
				thumbnail: index % 2 === 0 ? previews.city : previews.particles,
				detail: `${String(index + 3).padStart(2, "0")}:00 · 1920×1080`,
			})),
		],
	},
	{
		id: "folder-tour",
		name: "Tour Package",
		assets: [
			{
				id: "file-tour-titles",
				name: "Tour Titles",
				thumbnail: previews.titles,
				detail: "Still · 3840×2160",
			},
			{
				id: "file-night-forest",
				name: "Night Forest",
				thumbnail: previews.forest,
				detail: "00:42 · 3840×2160",
			},
		],
	},
	...Array.from({ length: 8 }, (_, index) => ({
		id: `folder-pack-${index + 1}`,
		name: [
			"Floral Patterns",
			"Abstract",
			"Architecture",
			"Atmospheres",
			"Brand Loops",
			"Concert",
			"Textures",
			"Utility",
		][index],
		assets: [
			{
				id: `file-pack-${index + 1}`,
				name: `${["Floral", "Abstract", "Architecture", "Atmosphere", "Brand", "Concert", "Texture", "Utility"][index]} 01`,
				thumbnail: index % 2 === 0 ? previews.forest : previews.titles,
				detail: "00:20 · 1920×1080",
			},
		],
	})),
];

export const dummyMaskFolders: DummyMediaFolder[] = [
	{
		id: "mask-shapes",
		name: "Shapes",
		assets: [
			{
				id: "mask-soft-circle",
				name: "Soft Circle",
				thumbnail: previews.mask,
				detail: "Mask · 1024×1024",
			},
			{
				id: "mask-checker",
				name: "Checker",
				thumbnail: previews.checker,
				detail: "Mask · 1024×1024",
			},
		],
	},
];

const readyLayers: DummyMediaLayer[] = [
	{
		id: "layer-main",
		name: "Layer 1 · Main",
		status: "ready",
		statusDetail: "Preview current",
		preview: previews.city,
		liveFolderId: "folder-city",
		liveFileId: "file-city-loop",
		maskFolderId: "mask-shapes",
		maskFileId: "mask-soft-circle",
	},
	{
		id: "layer-particles",
		name: "Layer 2 · Particles",
		status: "stale",
		statusDetail: "Last preview 18 seconds ago",
		preview: previews.particles,
		liveFolderId: "folder-city",
		liveFileId: "file-gold-particles",
		maskFolderId: null,
		maskFileId: null,
	},
	{
		id: "layer-titles",
		name: "Layer 3 · Titles",
		status: "failed",
		statusDetail: "Source failed; last frame retained",
		preview: previews.titles,
		liveFolderId: "folder-tour",
		liveFileId: "file-tour-titles",
		maskFolderId: "mask-shapes",
		maskFileId: "mask-checker",
	},
	...Array.from({ length: 5 }, (_, index) => ({
		id: `layer-${index + 4}`,
		name: `Layer ${index + 4} · ${["Background", "Lyrics", "Accents", "Camera", "Overlay"][index]}`,
		status: "ready" as const,
		statusDetail: "Preview current",
		preview: index % 2 === 0 ? previews.forest : previews.city,
		liveFolderId: index % 2 === 0 ? "folder-tour" : "folder-city",
		liveFileId: index % 2 === 0 ? "file-night-forest" : "file-city-loop",
		maskFolderId: index === 0 ? "mask-shapes" : null,
		maskFileId: index === 0 ? "mask-soft-circle" : null,
	})),
];

export const dummyMediaServers: DummyMediaServer[] = [
	{
		id: "server-aurora",
		name: "Aurora Media",
		address: "10.42.0.21 · Dummy",
		status: "ready",
		statusDetail: "Deterministic local presentation",
		programPreview: previews.program,
		layers: readyLayers,
		supportsMasks: true,
		supportsAudio: true,
		supportsEffects: true,
	},
	{
		id: "server-offline",
		name: "Tour Backup",
		address: "10.42.0.22 · Dummy",
		status: "offline",
		statusDetail: "Offline; last frame retained",
		programPreview: previews.forest,
		layers: [
			{
				...readyLayers[0],
				id: "backup-layer-main",
				status: "offline",
				statusDetail: "Server offline",
			},
		],
		supportsMasks: false,
		supportsAudio: false,
		supportsEffects: false,
	},
];

export const dummyMediaCapabilityStates: DummyMediaLayer[] = [
	readyLayers[0],
	readyLayers[1],
	readyLayers[2],
	{
		...readyLayers[0],
		id: "state-offline",
		name: "Offline server",
		status: "offline",
		statusDetail: "Connection unavailable; last frame retained",
	},
	{
		...readyLayers[0],
		id: "state-missing-patch",
		name: "Missing patch",
		status: "missing-patch",
		statusDetail: "Configured media master is no longer patched",
	},
	{
		...readyLayers[0],
		id: "state-unsupported",
		name: "Unsupported capability",
		status: "unsupported",
		statusDetail: "This layer does not advertise preview support",
	},
];
