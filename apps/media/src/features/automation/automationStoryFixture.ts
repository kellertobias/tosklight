/** Offline example data for the production playlist/schedule pages; never contacts a server. */
import thumbnail from "../../../../../docs/marketing/assets/stills/pixel-eight-layer-composite.png";
import {
	automationApi,
	type AutomationView,
	type Playlist,
} from "../../shared/api/automation";
import { api } from "../../shared/api/client";
import type {
	CatalogView,
	OutputView,
} from "../../shared/api/generated/media-wire";
import { resetResources } from "../../shared/api/resource";
import { DEFAULT_PLAYLIST_TRANSFORM } from "./playlistTransform";

const OUTPUT = "a20d74ef-5cb7-4c42-8df5-f39859864021";
const LOBBY = "lobby-information";
const catalog: CatalogView = {
	revision: 1,
	itemCount: 3,
	folders: [
		{
			folder: 1,
			name: "Venue information",
			items: [
				{
					id: "hours",
					file: 1,
					name: "Opening hours",
					kind: "image",
					width: 1920,
					height: 1080,
					frames: 1,
					durationMillis: 0,
					intrinsicBpm: null,
					enabled: true,
				},
				{
					id: "film",
					file: 2,
					name: "Venue film",
					kind: "video",
					width: 1920,
					height: 1080,
					frames: 750,
					durationMillis: 30000,
					intrinsicBpm: null,
					enabled: true,
				},
				{
					id: "logo",
					file: 3,
					name: "Venue logo",
					kind: "image",
					width: 1920,
					height: 1080,
					frames: 1,
					durationMillis: 0,
					intrinsicBpm: null,
					enabled: true,
				},
			],
		},
	],
};
const output: OutputView = {
	id: OUTPUT,
	name: "Lobby display",
	layerCount: 8,
	layers: [],
	dmxActive: false,
	playbackTakeover: false,
	ownershipGeneration: 1,
	frameRate: 25,
	master: {
		dimmer: 1,
		volume: 1,
		tintRed: 1,
		tintGreen: 1,
		tintBlue: 1,
		flipMirror: "none",
		mask: { folder: 0, file: 0, class: "blank" },
		maskPositionX: 0,
		maskPositionY: 0,
		scaleX: 1,
		scaleY: 1,
		scalingMode: "fit",
		positionX: 0,
		positionY: 0,
		rotation: 0,
		shaperLeft: 0,
		shaperRight: 0,
		shaperTop: 0,
		shaperBottom: 0,
		shaperLeftRotation: 0,
		shaperRightRotation: 0,
		shaperTopRotation: 0,
		shaperBottomRotation: 0,
		shaperRotation: 0,
		opacityCycle: "none",
		opacityCycleDmx: 0,
	},
};
function snapshot(): AutomationView {
	const playlist: Playlist = {
		id: LOBBY,
		name: "Lobby information",
		enabled: true,
		priority: 10,
		layers: [2, 3],
		masterDimmer: 1,
		masterVolume: 0,
		defaultFadeMillis: 1500,
		entries: [
			{
				id: "hours-entry",
				assetId: "hours",
				plays: 1,
				holdMillis: 15000,
				fadeMillis: 1500,
				transform: { ...DEFAULT_PLAYLIST_TRANSFORM },
			},
			{
				id: "film-entry",
				assetId: "film",
				plays: 2,
				holdMillis: null,
				fadeMillis: 1500,
				transform: { ...DEFAULT_PLAYLIST_TRANSFORM },
			},
		],
		auxiliaryLayers: [
			{
				layer: 4,
				mode: "fixed" as const,
				assetId: "logo",
				controls: {
					dimmer: 0.7,
					scaleX: 0.2,
					scaleY: 0.2,
					positionX: 0.7,
					positionY: -0.7,
					playMode: "loop",
				},
			},
		],
	};
	return {
		automationRevision: 1,
		timeZone: "CET (UTC+01:00)",
		configuration: {
			playlists: [
				playlist,
				{ ...playlist, id: "evening", name: "Evening welcome", priority: 20 },
				{ ...playlist, id: "fallback", name: "After hours", priority: 0 },
			],
			events: [
				{
					id: "weekday",
					name: "Weekday opening hours",
					enabled: true,
					outputId: OUTPUT,
					playlistId: LOBBY,
					trigger: {
						kind: "weekly",
						weekdays: [0, 1, 2, 3, 4],
						startTime: "09:00:00",
						endTime: "18:00:00",
						endDayOffset: 0,
						firstDate: null,
						lastDate: null,
					},
				},
				{
					id: "reception",
					name: "Monday reception",
					enabled: true,
					outputId: OUTPUT,
					playlistId: "evening",
					trigger: {
						kind: "fixed",
						start: "2026-11-23T17:00:00",
						end: "2026-11-23T19:00:00",
					},
				},
			],
			outputs: [
				{
					outputId: OUTPUT,
					enabled: true,
					fallbackPlaylistId: "fallback",
					dmxSilenceSeconds: 10,
				},
			],
		},
		status: [
			{
				outputId: OUTPUT,
				owner: "schedule",
				playlistId: LOBBY,
				eventId: "weekday",
				playlistName: "Lobby information",
				entryIndex: 1,
				play: 2,
				phase: "playing",
				error: null,
				dmxAgeMillis: null,
				resumeInMillis: null,
				lastOccurrenceOutcome: null,
				lastOccurrenceName: null,
			},
		],
		upcoming: [
			{
				eventId: "weekday",
				playlistId: LOBBY,
				outputId: OUTPUT,
				name: "Weekday opening hours",
				start: "2026-11-23T09:00:00+01:00",
				end: "2026-11-23T18:00:00+01:00",
				priority: 10,
				winning: true,
				reason: "Highest priority at start",
			},
			{
				eventId: "reception",
				playlistId: "evening",
				outputId: OUTPUT,
				name: "Monday reception",
				start: "2026-11-23T17:00:00+01:00",
				end: "2026-11-23T19:00:00+01:00",
				priority: 20,
				winning: true,
				reason: "Preempts weekday playlist",
			},
		],
	};
}

export function installAutomationExample() {
	resetResources();
	const view = snapshot();
	const originalApi = { ...api };
	const originalAutomation = { ...automationApi };
	const socket = Object.getOwnPropertyDescriptor(globalThis, "WebSocket");
	api.outputs = async () => [structuredClone(output)];
	api.catalog = async () => structuredClone(catalog);
	api.effects = async () => [];
	api.models = async () => [];
	api.thumbnailUrl = () => thumbnail;
	automationApi.read = async () => structuredClone(view);
	automationApi.updatePlaylist = async (id, change) => {
		Object.assign(
			view.configuration.playlists.find((p) => p.id === id)!,
			change,
		);
		return structuredClone(view);
	};
	automationApi.updateEvent = async (id, change) => {
		Object.assign(view.configuration.events.find((e) => e.id === id)!, change);
		return structuredClone(view);
	};
	automationApi.updateOutput = async (_id, change) => {
		Object.assign(view.configuration.outputs[0], change);
		return structuredClone(view);
	};
	automationApi.createPlaylist = async (value) => {
		view.configuration.playlists.push({ ...value, id: crypto.randomUUID() });
		return structuredClone(view);
	};
	automationApi.createEvent = async (value) => {
		view.configuration.events.push({ ...value, id: crypto.randomUUID() });
		return structuredClone(view);
	};
	automationApi.deletePlaylist = async (id) => {
		view.configuration.playlists = view.configuration.playlists.filter(
			(p) => p.id !== id,
		);
		return structuredClone(view);
	};
	automationApi.deleteEvent = async (id) => {
		view.configuration.events = view.configuration.events.filter(
			(e) => e.id !== id,
		);
		return structuredClone(view);
	};
	// In-memory telemetry keeps the ordinary UI connection and action paths functional offline.
	const sockets = new Set<ExampleSocket>();
	class ExampleSocket {
		static OPEN = 1;
		readyState = 1;
		onopen: (() => void) | null = null;
		onmessage: ((event: { data: string }) => void) | null = null;
		onclose: (() => void) | null = null;
		constructor() {
			sockets.add(this);
			queueMicrotask(() => {
				if (this.readyState === 1) {
					this.onopen?.();
					this.publish();
				}
			});
		}
		publish() {
			this.onmessage?.({
				data: JSON.stringify({
					automation: view.status,
					automationRevision: view.automationRevision,
				}),
			});
		}
		send(message: string) {
			const action = JSON.parse(message);
			if (action.type === "automation-set-enabled") {
				view.configuration.outputs[0].enabled = action.enabled;
				view.status[0].owner = action.enabled ? "schedule" : "disabled";
			}
			view.automationRevision++;
			this.onmessage?.({
				data: JSON.stringify({
					type: "automation-action-result",
					actionId: action.actionId,
					ok: true,
					message: null,
				}),
			});
			for (const subscriber of sockets) subscriber.publish();
		}
		close() {
			this.readyState = 3;
			sockets.delete(this);
		}
	}
	Object.defineProperty(globalThis, "WebSocket", {
		configurable: true,
		value: ExampleSocket,
	});
	return () => {
		Object.assign(api, originalApi);
		Object.assign(automationApi, originalAutomation);
		if (socket) Object.defineProperty(globalThis, "WebSocket", socket);
		else Reflect.deleteProperty(globalThis, "WebSocket");
		resetResources();
	};
}
