// The transport. One place converts an HTTP exchange into either a typed value or a typed
// failure, so no feature ever branches on a status code or reads `error.message` to decide what
// happened.

import type {
	ApiErrorBody,
	AudioPanelView,
	AudioSettingsView,
	CatalogView,
	CreateText,
	CreateVisualizer,
	DeleteText,
	DmxMapView,
	FolderPresentationView,
	FolderPresentationsView,
	Health,
	ImportsView,
	LogsView,
	NetworkView,
	OutputConfigurationView,
	OutputView,
	RunningServerView,
	ServerLogLevelView,
	StartImport,
	TextSlotView,
	TimeView,
	UpdateAudio,
	UpdateFolderPresentation,
	UpdateLayer,
	UpdateLibraryFolder,
	UpdateLibraryItem,
	UpdateMaster,
	UpdateNetwork,
	UpdateOutputConfiguration,
	UpdateServerLogLevel,
	UpdateText,
	UpdateTime,
	UpdateVisualizer,
	UploadAcceptedView,
	VisualizerView,
} from "./generated/media-wire";

/// Every failure a call site can see, including the ones that never reached the server.
export class ApiFailure extends Error {
	readonly code: string;
	readonly status: number;

	constructor(code: string, message: string, status: number) {
		super(message);
		this.name = "ApiFailure";
		this.code = code;
		this.status = status;
	}

	/** The server is not answering — a different situation from it answering "no". */
	get disconnected(): boolean {
		return this.status === 0;
	}

	/** A desk owns this output, so the web interface may not write to it right now. */
	get deskOwnsIt(): boolean {
		return (
			this.code === "dmx-owns-this" ||
			this.code === "playback-takeover-required"
		);
	}
}

const BASE = "/api/v2";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
	let response: Response;
	try {
		response = await fetch(`${BASE}${path}`, {
			...init,
			headers:
				init?.headers ??
				(init?.body && !(init.body instanceof FormData)
					? { "content-type": "application/json" }
					: undefined),
		});
	} catch {
		throw new ApiFailure(
			"unreachable",
			"the Media Server is not answering; check that it is running",
			0,
		);
	}

	if (!response.ok) throw await failureOf(response);
	if (response.status === 204) return undefined as T;
	return (await response.json()) as T;
}

async function failureOf(response: Response): Promise<ApiFailure> {
	// A route answers with the typed error body. Anything else — a proxy, a crash page — still
	// has to become a failure an operator can read.
	try {
		const body = (await response.json()) as ApiErrorBody;
		if (typeof body?.code === "string" && typeof body?.message === "string") {
			return new ApiFailure(body.code, body.message, response.status);
		}
	} catch {
		// fall through to the generic failure
	}
	return new ApiFailure(
		"unexpected-response",
		`the server answered ${response.status}`,
		response.status,
	);
}

const LAYER_U8_LIMITS = {
	folder: 255,
	file: 255,
	playModeDmx: 255,
	maskFolder: 255,
	maskFile: 255,
	speedMultiplierDmx: 255,
	playbackBpm: 255,
	effectSlot: 3,
} as const;

function boundedIntegerFields<T extends object>(
	edit: T,
	limits: Partial<Record<keyof T, number>>,
): T {
	const normalized = { ...edit } as unknown as Record<string, unknown>;
	for (const key of Object.keys(limits) as Array<keyof T & string>) {
		const value = normalized[key];
		if (value === undefined) continue;
		if (typeof value !== "number" || !Number.isFinite(value))
			throw new ApiFailure(
				"invalid-u8-control",
				`${String(key)} must be a finite integer`,
				400,
			);
		normalized[key] = Math.min(
			limits[key] ?? 255,
			Math.max(0, Math.round(value)),
		);
	}
	return normalized as T;
}

export const api = {
	health: () => request<Health>("/health"),
	runtime: () => request<RunningServerView>("/runtime"),
	catalog: () => request<CatalogView>("/catalog"),
	folderPresentations: () =>
		request<FolderPresentationsView>("/folder-presentations"),
	updateFolderPresentation: (folder: number, edit: UpdateFolderPresentation) =>
		request<FolderPresentationView>(`/folder-presentations/${folder}/update`, {
			method: "POST",
			body: JSON.stringify(edit),
		}),
	uploadFolderPicture: (folder: number, requestId: string, picture: File) => {
		const body = new FormData();
		body.set("file", picture);
		return request<FolderPresentationView>(
			`/folder-presentations/${folder}/picture/upload?${new URLSearchParams({ requestId })}`,
			{ method: "POST", body },
		);
	},
	removeFolderPicture: (folder: number, requestId: string) =>
		request<FolderPresentationView>(
			`/folder-presentations/${folder}/picture/remove`,
			{ method: "POST", body: JSON.stringify({ requestId }) },
		),
	visualizers: () => request<VisualizerView[]>("/visualizers"),
	createVisualizer: (edit: CreateVisualizer) =>
		request<VisualizerView>("/visualizers/create", {
			method: "POST",
			body: JSON.stringify(edit),
		}),
	fixtures: () => request<string[]>("/fixtures"),
	fixtureUrl: (name: string) => `${BASE}/fixtures/${encodeURIComponent(name)}`,
	outputs: () => request<OutputView[]>("/outputs"),
	outputState: (output: string) =>
		request<OutputView>(`/outputs/${output}/state`),
	outputPreviewUrl: (
		output: string,
		revision: number,
		size?: { width: number; height: number },
	) => {
		const query = new URLSearchParams({ revision: String(revision) });
		if (size) {
			const width = 640;
			query.set("width", String(width));
			query.set(
				"height",
				String(Math.max(1, Math.round((width * size.height) / size.width))),
			);
		}
		return `${BASE}/outputs/${output}/preview?${query.toString()}`;
	},
	outputLayerPreviewUrl: (
		output: string,
		layer: number,
		revision: number,
		size?: { width: number; height: number },
	) => {
		const query = new URLSearchParams({ revision: String(revision) });
		if (size) {
			const width = 320;
			query.set("width", String(width));
			query.set(
				"height",
				String(Math.max(1, Math.round((width * size.height) / size.width))),
			);
		}
		return `${BASE}/outputs/${output}/layers/${layer}/preview?${query.toString()}`;
	},
	setPlaybackTakeover: (output: string, takeOver: boolean) =>
		request<OutputView>(
			`/outputs/${output}/playback/${takeOver ? "take-over" : "release"}`,
		),
	outputConfiguration: (output: string) =>
		request<OutputConfigurationView>(`/outputs/${output}/configuration`),
	updateOutputConfiguration: (
		output: string,
		edit: UpdateOutputConfiguration,
	) =>
		request<OutputConfigurationView>(
			`/outputs/${output}/configuration/update`,
			{
				method: "POST",
				body: JSON.stringify(edit),
			},
		),
	dmxMap: (output: string) => request<DmxMapView>(`/outputs/${output}/dmx-map`),

	/** An intent-shaped write: only the fields being changed travel. */
	updateLayer: (output: string, layer: number, update: UpdateLayer) =>
		request<OutputView>(`/outputs/${output}/layers/${layer}/update`, {
			method: "POST",
			body: JSON.stringify(boundedIntegerFields(update, LAYER_U8_LIMITS)),
		}),
	updateMaster: (output: string, update: UpdateMaster) =>
		request<OutputView>(`/outputs/${output}/master/update`, {
			method: "POST",
			body: JSON.stringify(
				boundedIntegerFields(update, { maskFolder: 255, maskFile: 255 }),
			),
		}),

	/** An object-intent edit of stored configuration. Its request id makes a retry safe. */
	updateVisualizer: (folder: number, file: number, edit: UpdateVisualizer) =>
		request<VisualizerView>(`/visualizers/${folder}/${file}/update`, {
			method: "POST",
			body: JSON.stringify(edit),
		}),

	/** A live-control action with no payload, exactly as the API exposes it. */
	resetLayer: (output: string, layer: number) =>
		request<void>(`/outputs/${output}/layers/${layer}/reset`),

	imports: () => request<ImportsView>("/library/imports"),
	/** Long-running work, so this answers with jobs rather than holding the connection open. */
	startImport: (start: StartImport) =>
		request<ImportsView>("/library/import", {
			method: "POST",
			body: JSON.stringify(start),
		}),
	/** A payload-free action, exactly as the API exposes it. */
	cancelImport: (job: string) =>
		request<void>(`/library/imports/${job}/cancel`),
	updateLibraryItem: (id: string, edit: UpdateLibraryItem) =>
		request<CatalogView>(`/library/items/${encodeURIComponent(id)}/update`, {
			method: "POST",
			body: JSON.stringify(edit),
		}),
	updateLibraryFolder: (folder: number, edit: UpdateLibraryFolder) =>
		request<CatalogView>(`/library/folders/${folder}/update`, {
			method: "POST",
			body: JSON.stringify(edit),
		}),
	thumbnailUrl: (folder: number, file: number) =>
		`${BASE}/library/${folder}/${file}/thumbnail`,
	uploadLibraryItem: (
		folder: number,
		file: number,
		requestId: string,
		name: string,
		media: File,
		replace = false,
	) => {
		const query = new URLSearchParams({ requestId, name });
		if (replace) query.set("replace", "true");
		const body = new FormData();
		body.set("file", media);
		return request<UploadAcceptedView>(
			`/library/${folder}/${file}/upload?${query.toString()}`,
			{ method: "POST", body },
		);
	},

	network: () => request<NetworkView>("/network"),
	updateNetwork: (edit: UpdateNetwork) =>
		request<NetworkView>("/network/update", {
			method: "POST",
			body: JSON.stringify(edit),
		}),

	time: () => request<TimeView>("/time"),
	updateTime: (edit: UpdateTime) =>
		request<TimeView>("/time/update", {
			method: "POST",
			body: JSON.stringify(edit),
		}),

	audio: () => request<AudioPanelView>("/audio"),
	updateAudio: (edit: UpdateAudio) =>
		request<AudioSettingsView>("/audio/update", {
			method: "POST",
			body: JSON.stringify(edit),
		}),

	text: () => request<TextSlotView[]>("/text"),
	createText: (create: CreateText) =>
		request<TextSlotView>("/text/create", {
			method: "POST",
			body: JSON.stringify(create),
		}),
	updateText: (folder: number, file: number, edit: UpdateText) =>
		request<TextSlotView>(`/text/${folder}/${file}/update`, {
			method: "POST",
			body: JSON.stringify(edit),
		}),
	deleteText: (folder: number, file: number, remove: DeleteText) =>
		request<TextSlotView[]>(`/text/${folder}/${file}/delete`, {
			method: "POST",
			body: JSON.stringify(remove),
		}),

	/**
	 * A window of the log. The cursor is the newest record the viewer already holds, so a refresh
	 * cannot show one twice or step over one.
	 */
	logs: (query: { after?: number; level?: string; limit?: number } = {}) => {
		const parameters = new URLSearchParams();
		if (query.after !== undefined) parameters.set("after", String(query.after));
		if (query.level) parameters.set("level", query.level);
		if (query.limit !== undefined) parameters.set("limit", String(query.limit));
		const search = parameters.toString();
		return request<LogsView>(`/logs${search ? `?${search}` : ""}`);
	},
	serverLogLevel: () => request<ServerLogLevelView>("/logs/level"),
	updateServerLogLevel: (edit: UpdateServerLogLevel) =>
		request<ServerLogLevelView>("/logs/level/update", {
			method: "POST",
			body: JSON.stringify(edit),
		}),
};

/**
 * Where pushed telemetry arrives.
 *
 * Derived from the page's own origin, because the interface is served by the same process: an
 * operator who reached this server on a LAN address must not have a socket opened to a different
 * one. `https` pages get `wss`, so a reverse proxy in front of the server still works.
 */
export function telemetryUrl(): string {
	const { protocol, host } = window.location;
	return `${protocol === "https:" ? "wss" : "ws"}://${host}${BASE}/telemetry`;
}

export type MediaApi = typeof api;
