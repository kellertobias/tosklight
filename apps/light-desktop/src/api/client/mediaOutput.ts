import type {
	OutputRuntimeActionOutcome,
	OutputRuntimeActionRequest,
} from "../../features/outputRuntime/contracts";
import type {
	DmxOverrideRequest,
	DiscoveredMediaAddressUpdateRequest,
	DiscoveredMediaOutput,
	DiscoveredMediaServer,
	HighlightActionRequest,
	MediaLibrarySelectionOutcome,
	MediaLibrarySelectionRequest,
	MediaPreviewRefreshRequest,
	MediaServerDiscovery,
	MediaThumbnailRefreshRequest,
	NativeMediaEffectSlot as NativeMediaEffectSlotWire,
	NativeMediaEffectUpdateRequest,
	NativeMediaSnapshot as NativeMediaSnapshotWire,
	NativeMediaTextSlot as NativeMediaTextSlotWire,
	NativeMediaTextUpdateRequest,
	PatchPreviewHighlightRequest,
} from "../generated/light-wire";
export type {
	DiscoveredMediaOutput,
	DiscoveredMediaServer,
	MediaServerDiscovery,
} from "../generated/light-wire";
import {
	decodeOutputRuntimeActionOutcome,
	encodeOutputRuntimeActionRequest,
} from "../outputRuntimeWire";
import type {
	DmxSnapshot,
	HighlightAction,
	HighlightState,
	MediaServerFixture,
	VisualizationSnapshot,
} from "../types";
import type { LiveClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export interface MediaPreviewRefresh {
	fixture_id: string;
	source: number;
	format: string;
	width: number;
	height: number;
}

export interface NativeMediaTextSlot {
	folder: number;
	file: number;
	name: string;
	enabled: boolean;
	kind: string;
	text: string | null;
}

export interface NativeMediaSnapshot {
	endpoint: string;
	status: string;
	instance: string;
	outputs: number;
	catalogRevision: number;
	catalogItems: number;
	textSlots: NativeMediaTextSlot[];
	effectControlsAvailable: boolean;
	outputId: string | null;
	effectLayers: NativeMediaEffectSlot[][];
}

export interface NativeMediaEffectParameter {
	id: string;
	label: string;
	value: number;
	defaultValue: number;
}

export interface NativeMediaEffectSlot {
	index: number;
	effectType: string | null;
	label: string;
	enabled: boolean;
	mix: number;
	supported: boolean;
	capabilityDetail: string | null;
	parameters: NativeMediaEffectParameter[];
}

export interface MediaServerInspection {
	library_revision: string;
	server: { name: string; layer_count: number };
	folders: Array<{ id: number; name: string; element_count: number }>;
	files: Array<{
		folder_id: number;
		id: number;
		name: string;
		width: number;
		height: number;
		length_frames: number;
		fps: number;
	}>;
	preview_sources: Array<{
		id: number;
		name: string;
		physical_output: number;
		layer: number | null;
		width: number;
		height: number;
	}>;
	layers: Array<{
		layer: number;
		physical_output: number;
		folder: number;
		file: number;
		name: string;
		position_frames: number;
		length_frames: number;
		fps: number;
		flags: number;
	}>;
	capabilities: {
		provider: string;
		native_action: string | null;
		layers: Array<{
			layer: number;
			content_library: boolean;
			mask_library: boolean;
			secondary_controls: Array<{ attribute: string }>;
		}>;
	};
}

export class MediaOutputApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	visualization(preload = false): Promise<VisualizationSnapshot> {
		const query = preload ? "?preload=true" : "";
		return this.transport.request(`/api/v2/output/visualization${query}`);
	}

	dmx(): Promise<DmxSnapshot> {
		return this.transport.request("/api/v2/output/dmx", {}, false);
	}

	mediaServers(): Promise<{ fixtures: MediaServerFixture[] }> {
		return this.transport.request("/api/v2/media-servers");
	}

	discoverMediaServers(): Promise<MediaServerDiscovery> {
		return this.transport.request("/api/v2/media-servers/discover");
	}

	updateDiscoveredMediaAddress(input: {
		host: string;
		outputId: string;
		universe: number;
		startAddress: number;
	}): Promise<DiscoveredMediaOutput> {
		const request: DiscoveredMediaAddressUpdateRequest = {
			requestId: crypto.randomUUID(),
			...input,
		};
		return this.transport.request(
			"/api/v2/media-servers/discovered/address",
			jsonRequest("POST", request),
		);
	}

	inspectMediaServer(fixtureId: string): Promise<MediaServerInspection> {
		return this.transport.request(`/api/v2/media-servers/${fixtureId}/inspect`);
	}

	nativeMedia(fixtureId: string): Promise<NativeMediaSnapshot> {
		return this.transport
			.request<NativeMediaSnapshotWire>(
				`/api/v2/media-servers/${fixtureId}/native`,
			)
			.then(mapNativeMediaSnapshot);
	}

	updateNativeMediaText(
		fixtureId: string,
		folder: number,
		file: number,
		text: string,
	): Promise<NativeMediaTextSlot> {
		const request: NativeMediaTextUpdateRequest = {
			request_id: crypto.randomUUID(),
			text,
		};
		return this.transport
			.request<NativeMediaTextSlotWire>(
				`/api/v2/media-servers/${fixtureId}/native/text/${folder}/${file}/update`,
				jsonRequest("POST", request),
			)
			.then(mapNativeMediaTextSlot);
	}

	updateNativeMediaEffect(
		fixtureId: string,
		layer: number,
		controlId: string,
		value: string | number | boolean,
	): Promise<NativeMediaEffectSlot[]> {
		const typedValue =
			controlId.endsWith("-enabled") && typeof value === "string"
				? value === "true"
				: value;
		const request: NativeMediaEffectUpdateRequest = {
			request_id: crypto.randomUUID(),
			control_id: controlId,
			number_value: typeof typedValue === "number" ? typedValue : null,
			string_value: typeof typedValue === "string" ? typedValue : null,
			boolean_value: typeof typedValue === "boolean" ? typedValue : null,
		};
		return this.transport
			.request<NativeMediaEffectSlotWire[]>(
				`/api/v2/media-servers/${fixtureId}/native/layers/${layer}/effects/update`,
				jsonRequest("POST", request),
			)
			.then((slots) => slots.map(mapNativeMediaEffectSlot));
	}

	applyMediaLibrarySelection(
		fixtureId: string,
		input: Omit<MediaLibrarySelectionRequest, "request_id">,
	): Promise<MediaLibrarySelectionOutcome> {
		const request: MediaLibrarySelectionRequest = {
			...input,
			request_id: crypto.randomUUID(),
		};
		return this.transport.request(
			`/api/v2/media-servers/${fixtureId}/library-selection`,
			jsonRequest("POST", request),
		);
	}

	refreshMediaPreview(
		fixtureId: string,
		source = 0,
		width = 320,
		height = 180,
	): Promise<MediaPreviewRefresh> {
		const request = {
			source,
			width,
			height,
		} satisfies MediaPreviewRefreshRequest;
		return this.transport.request(
			`/api/v2/media-servers/${fixtureId}/preview/refresh`,
			jsonRequest("POST", request),
		);
	}

	mediaPreview(fixtureId: string, source = 0): Promise<Blob> {
		return this.transport.blob(
			`/api/v2/media-servers/${fixtureId}/preview/${source}`,
		);
	}

	mediaThumbnail(
		fixtureId: string,
		folder: number,
		element: number,
	): Promise<Blob> {
		return this.transport.blob(
			`/api/v2/media-servers/${fixtureId}/thumbnails/${folder}/${element}`,
		);
	}

	refreshMediaThumbnails(
		fixtureId: string,
		folder: number,
		elements: number[],
		width = 128,
		height = 72,
	): Promise<{ fixture_id: string; count: number }> {
		const request = {
			library_type: 1,
			library_level: 1,
			library_1: folder,
			library_2: 0,
			library_3: 0,
			elements,
			width,
			height,
		} satisfies MediaThumbnailRefreshRequest;
		return this.transport.request(
			`/api/v2/media-servers/${fixtureId}/thumbnails/refresh`,
			jsonRequest("POST", request),
		);
	}

	async outputRuntimeLiveAction(
		showId: string,
		request: OutputRuntimeActionRequest,
	): Promise<OutputRuntimeActionOutcome> {
		const wireRequest = encodeOutputRuntimeActionRequest(request);
		const value = await this.transport.sendAction(
			{ type: "output_runtime", request: wireRequest },
			wireRequest.request_id,
		);
		return decodeOutputRuntimeActionOutcome(value, showId, request);
	}

	setDmxOverride(universe: number, address: number, value: number | null) {
		const requestId = crypto.randomUUID();
		const request = {
			request_id: requestId,
			universe,
			address,
			value,
		} satisfies DmxOverrideRequest;
		return this.transport.sendAction(
			{ type: "dmx_override", request },
			requestId,
		);
	}

	highlight(): Promise<HighlightState> {
		return this.transport.request("/api/v2/output/highlight");
	}

	highlightAction(action: HighlightAction): Promise<HighlightState> {
		const requestId = crypto.randomUUID();
		const request = {
			request_id: requestId,
			action,
		} satisfies HighlightActionRequest;
		return this.transport.sendAction(
			{ type: "highlight", request },
			requestId,
		) as Promise<HighlightState>;
	}

	setPatchPreviewHighlight(active: boolean, fixtureIds: string[] = []) {
		const requestId = crypto.randomUUID();
		const request = {
			request_id: requestId,
			active,
			fixture_ids: fixtureIds,
		} satisfies PatchPreviewHighlightRequest;
		return this.transport.sendAction(
			{ type: "patch_preview_highlight", request },
			requestId,
		) as Promise<{ active: boolean; allowed: boolean }>;
	}
}

function mapNativeMediaTextSlot(
	slot: NativeMediaTextSlotWire,
): NativeMediaTextSlot {
	return { ...slot, text: slot.text ?? null };
}

function mapNativeMediaSnapshot(
	snapshot: NativeMediaSnapshotWire,
): NativeMediaSnapshot {
	return {
		endpoint: snapshot.endpoint,
		status: snapshot.status,
		instance: snapshot.instance,
		outputs: snapshot.outputs,
		catalogRevision: snapshot.catalog_revision,
		catalogItems: snapshot.catalog_items,
		textSlots: snapshot.text_slots.map(mapNativeMediaTextSlot),
		effectControlsAvailable: snapshot.effect_controls_available,
		outputId: snapshot.output_id ?? null,
		effectLayers: (snapshot.effect_layers ?? []).map((layer) =>
			layer.map(mapNativeMediaEffectSlot),
		),
	};
}

function mapNativeMediaEffectSlot(
	slot: NativeMediaEffectSlotWire,
): NativeMediaEffectSlot {
	return {
		index: slot.index,
		effectType: slot.effect_type ?? null,
		label: slot.label,
		enabled: slot.enabled,
		mix: slot.mix,
		supported: slot.supported,
		capabilityDetail: slot.capability_detail ?? null,
		parameters: slot.parameters.map((parameter) => ({
			id: parameter.id,
			label: parameter.label,
			value: parameter.value,
			defaultValue: parameter.default_value,
		})),
	};
}
