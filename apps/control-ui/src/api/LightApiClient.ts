import {
	type PresetAddress,
	type PresetFamily,
	presetStorageKey,
} from "../presetFamilies";
import {
	ConfigurationApiClient,
	type DeskLockInput,
} from "./client/configuration";
import { FileApiClient, type FileOperationInput } from "./client/files";
import { FixtureApiClient } from "./client/fixtures";
import { MediaApiClient } from "./client/media";
import {
	type MvrApplyInput,
	ShowApiClient,
	type ShowOpenTransition,
} from "./client/shows";
import type { ClientTransport } from "./client/transport";
import type {
	BootstrapSnapshot,
	CommandHistoryEntry,
	DeskConfiguration,
	DmxSnapshot,
	HelpCatalog,
	HelpTopic,
	PatchSnapshot,
	PlaybackSnapshot,
	ScreenConfiguration,
	ScreenSnapshot,
	ServerEvent,
	SessionResponse,
	ShowEntry,
	VersionedObject,
} from "./types";

type EventListener = (event: ServerEvent) => void;

function persistentBrowserStorage(): Storage | null {
	const storage = globalThis.localStorage;
	return storage && typeof storage.getItem === "function" ? storage : null;
}

function browserSessionStorage(): Storage | null {
	const storage = globalThis.sessionStorage;
	return storage && typeof storage.getItem === "function" ? storage : null;
}

function browserStorage(): Storage | null {
	const session = browserSessionStorage();
	return session?.getItem("light.test-server-url")
		? session
		: persistentBrowserStorage();
}

interface CommandResponse {
	protocol_version: number;
	request_id: string;
	ok: boolean;
	revision: number;
	payload?: unknown;
	error?: string;
}

export function defaultServerUrl(location = window.location): string {
	const configured = import.meta.env.VITE_LIGHT_SERVER_URL as
		| string
		| undefined;
	if (configured) return configured.replace(/\/$/, "");
	if (location.protocol === "tauri:")
		return (
			browserSessionStorage()?.getItem("light.test-server-url") ||
			persistentBrowserStorage()?.getItem("light.server-url") ||
			"http://127.0.0.1:5000"
		).replace(/\/$/, "");
	return location.origin;
}

export function configuredServerUrl() {
	return defaultServerUrl();
}
export function saveServerUrl(value: string) {
	const url = new URL(value.trim());
	if (url.protocol !== "http:" && url.protocol !== "https:")
		throw new Error("Server URL must use http or https");
	browserStorage()?.setItem(
		"light.server-url",
		url.toString().replace(/\/$/, ""),
	);
}

export class LightApiClient {
	private session: SessionResponse | null = null;
	private socket: WebSocket | null = null;
	private listeners = new Set<EventListener>();
	private pending = new Map<
		string,
		{
			resolve: (value: unknown) => void;
			reject: (error: Error) => void;
			timer: number;
		}
	>();
	private deskToken = browserStorage()?.getItem("light.desk-token") ?? "";
	private readonly fileApi: FileApiClient;
	private readonly fixtureApi: FixtureApiClient;
	private readonly mediaApi: MediaApiClient;
	private readonly showApi: ShowApiClient;
	private readonly configurationApi: ConfigurationApiClient;

	constructor(private readonly baseUrl = defaultServerUrl()) {
		const transport: ClientTransport = {
			request: <T>(path: string, init?: RequestInit, authenticate?: boolean) =>
				this.request<T>(path, init, authenticate),
			blob: (path: string, init?: RequestInit) => this.requestBlob(path, init),
			absoluteUrl: (path: string) => `${this.baseUrl}${path}`,
		};
		this.fileApi = new FileApiClient(transport);
		this.fixtureApi = new FixtureApiClient(transport);
		this.mediaApi = new MediaApiClient(transport);
		this.showApi = new ShowApiClient(transport);
		this.configurationApi = new ConfigurationApiClient(transport);
	}

	helpCatalog(): Promise<HelpCatalog> {
		return this.request("/api/v1/help", {}, false);
	}
	helpTopic(id: string): Promise<HelpTopic> {
		return this.request(
			`/api/v1/help/topics/${encodeURIComponent(id)}`,
			{},
			false,
		);
	}
	commandHistory(): Promise<CommandHistoryEntry[]> {
		return this.request("/api/v1/command-history");
	}
	fileRoots(): Promise<import("./types").FileRoot[]> {
		return this.fileApi.fileRoots();
	}
	fileEntries(
		root: string,
		path = "",
		hidden = false,
	): Promise<import("./types").FileDirectory> {
		return this.fileApi.fileEntries(root, path, hidden);
	}
	fileMetadata(
		root: string,
		path: string,
	): Promise<import("./types").FileMetadata> {
		return this.fileApi.fileMetadata(root, path);
	}
	readFileNote(
		root: string,
		path: string,
	): Promise<import("./types").FileNativeNote> {
		return this.fileApi.readFileNote(root, path);
	}
	saveFileNote(
		root: string,
		path: string,
		note: string,
	): Promise<import("./types").FileNativeNote> {
		return this.fileApi.saveFileNote(root, path, note);
	}
	readTextFile(
		root: string,
		path: string,
	): Promise<import("./types").TextDocument> {
		return this.fileApi.readTextFile(root, path);
	}
	saveTextFile(
		root: string,
		path: string,
		text: string,
		revision: string | null,
	): Promise<import("./types").TextDocument> {
		return this.fileApi.saveTextFile(root, path, text, revision);
	}
	fileOperation(root: string, input: FileOperationInput) {
		return this.fileApi.fileOperation(root, input);
	}
	fileContent(root: string, path: string): Promise<Blob> {
		return this.fileApi.fileContent(root, path);
	}
	fileStreamUrl(root: string, path: string): Promise<string> {
		return this.fileApi.fileStreamUrl(root, path);
	}
	fileThumbnail(root: string, path: string, maxSize = 256): Promise<Blob> {
		return this.fileApi.fileThumbnail(root, path, maxSize);
	}
	claimFileInput(
		instanceId: string,
		action: import("./types").FileInputAction,
		origin: "pending" | "toolbar",
	): Promise<import("./types").FileInputContext> {
		return this.fileApi.claimFileInput(instanceId, action, origin);
	}
	releaseFileInput(instanceId: string): Promise<void> {
		return this.fileApi.releaseFileInput(instanceId);
	}

	get currentSession() {
		return this.session;
	}
	restoreSession(session: SessionResponse) {
		this.session = session;
	}
	setDeskToken(token: string) {
		this.deskToken = token.trim();
		const storage = browserStorage();
		if (this.deskToken) storage?.setItem("light.desk-token", this.deskToken);
		else storage?.removeItem("light.desk-token");
	}
	private boundaryHeaders(headers = new Headers()) {
		if (this.deskToken) headers.set("x-light-desk-token", this.deskToken);
		return headers;
	}

	async bootstrap(): Promise<BootstrapSnapshot> {
		return this.request("/api/v1/bootstrap", {}, false);
	}

	async login(username: string): Promise<SessionResponse> {
		const storage = browserStorage();
		let clientId = storage?.getItem("light.client-id");
		if (!clientId) {
			clientId = crypto.randomUUID();
			storage?.setItem("light.client-id", clientId);
		}
		const session = await this.request<SessionResponse>(
			"/api/v1/sessions",
			{
				method: "POST",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({
					username,
					client_id: clientId,
					desk_id: storage?.getItem("light.control-desk") ?? null,
				}),
			},
			false,
		);
		this.session = session;
		storage?.setItem("light.primary-session", JSON.stringify(session));
		if (session.desk) storage?.setItem("light.control-desk", session.desk.id);
		return session;
	}

	async closeSession() {
		const session = this.session;
		if (!session) return;
		try {
			const response = await fetch(
				`${this.baseUrl}/api/v1/sessions/${session.session_id}`,
				{
					method: "DELETE",
					keepalive: true,
					headers: this.boundaryHeaders(
						new Headers({ authorization: `Bearer ${session.token}` }),
					),
				},
			);
			if (!response.ok && response.status !== 404)
				throw new Error(await response.text());
		} finally {
			if (this.session?.session_id === session.session_id) this.session = null;
		}
	}

	createUser(name: string): Promise<import("./types").DeskUser> {
		return this.request("/api/v1/users", {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ name, enabled: true }),
		});
	}

	patch(): Promise<PatchSnapshot> {
		return this.fixtureApi.patch();
	}

	fixtureLibrary(): Promise<import("./types").FixtureDefinition[]> {
		return this.fixtureApi.fixtureLibrary();
	}

	fixtureProfiles(): Promise<import("./types").FixtureProfile[]> {
		return this.fixtureApi.fixtureProfiles();
	}

	fixtureProfileWarnings(): Promise<string[]> {
		return this.fixtureApi.fixtureProfileWarnings();
	}

	fixtureProfileRevisions(
		id: string,
	): Promise<import("./types").FixtureProfile[]> {
		return this.fixtureApi.fixtureProfileRevisions(id);
	}

	putFixtureProfile(
		profile: import("./types").FixtureProfile,
		expectedRevision: number,
	) {
		return this.fixtureApi.putFixtureProfile(profile, expectedRevision);
	}

	deleteFixtureProfile(id: string, revision: number) {
		return this.fixtureApi.deleteFixtureProfile(id, revision);
	}

	putFixtureProfileSourceGdtf(
		id: string,
		revision: number,
		source: Uint8Array,
	) {
		return this.fixtureApi.putFixtureProfileSourceGdtf(id, revision, source);
	}

	importFixturePackage(source: Uint8Array) {
		return this.fixtureApi.importFixturePackage(source);
	}

	exportFixturePackage(id: string, revision: number): Promise<Blob> {
		return this.fixtureApi.exportFixturePackage(id, revision);
	}

	putFixtureDefinition(definition: import("./types").FixtureDefinition) {
		return this.fixtureApi.putFixtureDefinition(definition);
	}

	deleteFixtureDefinition(id: string, revision: number) {
		return this.fixtureApi.deleteFixtureDefinition(id, revision);
	}

	playbacks(): Promise<PlaybackSnapshot> {
		return this.request("/api/v1/playbacks");
	}
	screens(): Promise<ScreenSnapshot> {
		return this.request("/api/v1/screens");
	}
	putScreen(screen: ScreenConfiguration): Promise<ScreenConfiguration> {
		return this.request(`/api/v1/screens/${screen.id}`, {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(screen),
		});
	}
	deleteScreen(id: string): Promise<void> {
		return this.request(`/api/v1/screens/${id}`, { method: "DELETE" });
	}
	setScreenPage(id: string, page: number) {
		return this.request(`/api/v1/screens/${id}/page`, {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ page }),
		});
	}

	visualization(
		preload = false,
	): Promise<import("./types").VisualizationSnapshot> {
		return this.mediaApi.visualization(preload);
	}

	dmx(): Promise<DmxSnapshot> {
		return this.mediaApi.dmx();
	}

	mediaServers(): Promise<{
		fixtures: import("./types").MediaServerFixture[];
	}> {
		return this.mediaApi.mediaServers();
	}

	refreshMediaPreview(
		fixtureId: string,
		source = 0,
		width = 320,
		height = 180,
	) {
		return this.mediaApi.refreshMediaPreview(fixtureId, source, width, height);
	}

	mediaPreview(fixtureId: string, source = 0): Promise<Blob> {
		return this.mediaApi.mediaPreview(fixtureId, source);
	}

	refreshMediaThumbnails(
		fixtureId: string,
		elements: number[],
		width = 128,
		height = 72,
	) {
		return this.mediaApi.refreshMediaThumbnails(
			fixtureId,
			elements,
			width,
			height,
		);
	}

	shows(): Promise<ShowEntry[]> {
		return this.showApi.shows();
	}

	createShow(
		name: string,
		dataBase64: string | null = null,
		overwrite = false,
	): Promise<ShowEntry> {
		return this.showApi.createShow(name, dataBase64, overwrite);
	}

	openShow(
		id: string,
		transition: ShowOpenTransition = "safe_blackout",
		transitionMillis?: number,
	): Promise<ShowEntry> {
		return this.showApi.openShow(id, transition, transitionMillis);
	}

	openCleanDefaultShow(): Promise<ShowEntry> {
		return this.showApi.openCleanDefaultShow();
	}

	renameShow(id: string, name: string): Promise<ShowEntry> {
		return this.showApi.renameShow(id, name);
	}

	overwriteShow(sourceId: string, destinationId: string): Promise<ShowEntry> {
		return this.showApi.overwriteShow(sourceId, destinationId);
	}

	showRevisions(id: string): Promise<import("./types").ShowRevision[]> {
		return this.showApi.showRevisions(id);
	}

	saveShowRevision(
		id: string,
		name: string,
	): Promise<import("./types").ShowRevision> {
		return this.showApi.saveShowRevision(id, name);
	}

	openShowRevision(id: string, revision: number): Promise<ShowEntry> {
		return this.showApi.openShowRevision(id, revision);
	}

	rollbackShow(): Promise<ShowEntry> {
		return this.showApi.rollbackShow();
	}

	downloadShow(id: string): Promise<Blob> {
		return this.showApi.downloadShow(id);
	}

	previewMvr(
		file: File,
		showId?: string,
	): Promise<import("./types").MvrImportPreview> {
		return this.showApi.previewMvr(file, showId);
	}

	applyMvr(token: string, input: MvrApplyInput) {
		return this.showApi.applyMvr(token, input);
	}

	mvrExportPreview(id: string): Promise<import("./types").MvrExportPreview> {
		return this.showApi.mvrExportPreview(id);
	}
	downloadMvr(id: string): Promise<Blob> {
		return this.showApi.downloadMvr(id);
	}

	configuration(): Promise<{
		configuration: DeskConfiguration;
		output_health: import("./types").OutputHealth;
		matter: import("./types").MatterBridgeStatus;
	}> {
		return this.configurationApi.configuration();
	}

	updateConfiguration(configuration: DeskConfiguration): Promise<{
		configuration: DeskConfiguration;
		requires_restart: boolean;
		matter: import("./types").MatterBridgeStatus;
	}> {
		return this.configurationApi.updateConfiguration(configuration);
	}

	matterStatus(): Promise<import("./types").MatterBridgeStatus> {
		return this.configurationApi.matterStatus();
	}

	speedGroup(
		group: import("./types").SpeedGroupId,
	): Promise<import("./types").SpeedGroupSoundState> {
		return this.configurationApi.speedGroup(group);
	}

	updateSpeedGroup(
		group: import("./types").SpeedGroupId,
		configuration: import("./types").SoundToLightConfig,
	): Promise<import("./types").SpeedGroupSoundState> {
		return this.configurationApi.updateSpeedGroup(group, configuration);
	}

	observeSpeedGroup(
		group: import("./types").SpeedGroupId,
		observation: import("./types").SoundObservation,
	): Promise<import("./types").SpeedGroupSoundState> {
		return this.configurationApi.observeSpeedGroup(group, observation);
	}

	speedGroupAction(
		group: import("./types").SpeedGroupId,
		input: import("./types").SpeedGroupActionInput,
	): Promise<import("./types").SpeedGroupSoundState> {
		return this.configurationApi.speedGroupAction(group, input);
	}

	shutdown(): Promise<{ shutting_down: boolean }> {
		return this.configurationApi.shutdown();
	}

	deskLock(): Promise<import("./types").DeskLockState> {
		return this.configurationApi.deskLock();
	}
	configureDeskLock(input: DeskLockInput) {
		return this.configurationApi.configureDeskLock(input);
	}
	lockDesk(): Promise<import("./types").DeskLockState> {
		return this.configurationApi.lockDesk();
	}
	unlockDesk(pin?: string): Promise<import("./types").DeskLockState> {
		return this.configurationApi.unlockDesk(pin);
	}

	objects<T>(showId: string, kind: string): Promise<VersionedObject<T>[]> {
		return this.request(
			`/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}`,
			{},
			false,
		);
	}

	putObject<T>(
		showId: string,
		kind: string,
		id: string,
		body: T,
		revision: number,
	): Promise<{ revision: number }> {
		return this.request(
			`/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}`,
			{
				method: "PUT",
				headers: {
					"content-type": "application/json",
					"if-match": String(revision),
				},
				body: JSON.stringify(body),
			},
		);
	}

	deleteObject(
		showId: string,
		kind: string,
		id: string,
		revision: number,
	): Promise<void> {
		return this.request(
			`/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}`,
			{
				method: "DELETE",
				headers: { "if-match": String(revision) },
			},
		);
	}

	setDmxOverride(universe: number, address: number, value: number | null) {
		return this.request("/api/v1/dmx/override", {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ universe, address, value }),
		});
	}

	programmers() {
		return this.request<import("./types").ProgrammerState[]>(
			"/api/v1/programmers",
			{},
			false,
		);
	}

	highlight() {
		return this.request<import("./types").HighlightState>("/api/v1/highlight");
	}

	highlightAction(action: import("./types").HighlightAction) {
		return this.request<import("./types").HighlightState>(
			"/api/v1/highlight/action",
			{
				method: "POST",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({ action }),
			},
		);
	}

	setPatchPreviewHighlight(active: boolean, fixtureIds: string[] = []) {
		return this.request<{ active: boolean; allowed: boolean }>(
			"/api/v1/patch-preview-highlight",
			{
				method: "PUT",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({ active, fixture_ids: fixtureIds }),
			},
		);
	}

	updateSettings() {
		return this.request<import("./types").UpdateSettings>(
			"/api/v1/update/settings",
		);
	}

	saveUpdateSettings(settings: import("./types").UpdateSettings) {
		return this.request<import("./types").UpdateSettings>(
			"/api/v1/update/settings",
			{
				method: "PUT",
				headers: { "content-type": "application/json" },
				body: JSON.stringify(settings),
			},
		);
	}

	previewUpdate(
		target: import("./types").UpdateTargetRequest,
		mode: import("./types").UpdateMode,
	) {
		return this.request<import("./types").UpdatePreview>(
			"/api/v1/update/preview",
			{
				method: "POST",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({ target, mode }),
			},
		);
	}

	applyUpdate(
		target: import("./types").UpdateTargetRequest,
		mode: import("./types").UpdateMode,
		expectedRevision?: number,
		expectedProgrammerRevision?: string,
	) {
		return this.request<import("./types").UpdateResult>(
			"/api/v1/update/apply",
			{
				method: "POST",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({
					target,
					mode,
					...(expectedRevision == null
						? {}
						: { expected_revision: expectedRevision }),
					...(expectedProgrammerRevision == null
						? {}
						: { expected_programmer_revision: expectedProgrammerRevision }),
				}),
			},
		);
	}

	updateTargets(filter: import("./types").UpdateTargetFilter) {
		return this.request<import("./types").UpdateMenuEntry[]>(
			`/api/v1/update/targets?filter=${encodeURIComponent(filter)}`,
		);
	}

	auditEvents(after = 0) {
		return this.request<
			Array<{ revision: number; kind: string; payload: unknown }>
		>(`/api/v1/audit?after=${after}`);
	}

	clearProgrammer(sessionId: string) {
		return this.request(`/api/v1/programmers/${sessionId}/clear`, {
			method: "POST",
		});
	}
	clearProgrammerValues() {
		return this.command("programmer.clear", {});
	}

	selectGroup(
		groupId: string,
		frozen = false,
		rule: Record<string, unknown> = { type: "all" },
	) {
		return this.command("group.select", { group_id: groupId, frozen, rule });
	}

	selectionMacro(rule: Record<string, unknown>) {
		return this.command("selection.macro", { rule });
	}

	align(
		attribute: string,
		mode: "left" | "right" | "center" | "out",
		from = 0,
		to = 1,
	) {
		return this.command("programmer.align", { attribute, mode, from, to });
	}
	preload(action: "enter" | "go" | "clear" | "release") {
		return this.command(`preload.${action}`, {});
	}
	setPreloadGroup(groupId: string, attribute: string, value: number) {
		return this.command("preload.group.set", {
			group_id: groupId,
			attribute,
			value,
		});
	}
	storePreload(
		showId: string,
		input: {
			target: "preset" | "cue";
			target_id: string;
			cue_number?: number;
			name?: string;
			mode?: "merge" | "overwrite" | "add_missing_fixtures";
			family?: PresetFamily;
		},
		revision: number,
	) {
		return this.request(`/api/v1/shows/${showId}/preload/store`, {
			method: "POST",
			headers: {
				"content-type": "application/json",
				"if-match": String(revision),
			},
			body: JSON.stringify(input),
		});
	}

	undoObject(showId: string, kind: string, id: string, revision: number) {
		return this.request(
			`/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}/undo`,
			{ method: "POST", headers: { "if-match": String(revision) } },
		);
	}

	playbackAction(
		cueListId: string,
		action: "go" | "back" | "pause" | "release",
	) {
		return this.command(`playback.${action}`, { cue_list_id: cueListId });
	}
	poolPlaybackAction(
		number: number,
		action:
			| "button"
			| "on"
			| "off"
			| "toggle"
			| "go"
			| "go-minus"
			| "go-to"
			| "load"
			| "fast-forward"
			| "fast-rewind"
			| "temp"
			| "temp-on"
			| "temp-off"
			| "swap"
			| "select"
			| "select-contents"
			| "select-dereferenced"
			| "learn"
			| "double"
			| "half"
			| "pause"
			| "blackout"
			| "pause-dynamics"
			| "flash"
			| "master"
			| "xfade-on"
			| "xfade-off",
		input: {
			value?: number;
			pressed?: boolean;
			button?: number;
			cue_number?: number;
			surface?: "physical" | "virtual";
		} = {},
	) {
		return this.request(`/api/v1/cuelists/${number}/${action}`, {
			method: action === "master" ? "PUT" : "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(input),
		});
	}
	virtualPlaybackExclusionZones() {
		return this.request<import("./types").VirtualPlaybackExclusionSnapshot>(
			"/api/v1/virtual-playback-exclusion-zones",
		);
	}
	saveVirtualPlaybackExclusionZones(
		surfaceId: string,
		zones: import("./types").VirtualPlaybackExclusionZone[],
	) {
		return this.request<{
			surface_id: string;
			zones: import("./types").VirtualPlaybackExclusionZone[];
		}>(
			`/api/v1/virtual-playback-exclusion-zones/${encodeURIComponent(surfaceId)}`,
			{
				method: "PUT",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({ zones }),
			},
		);
	}
	savePlaybackSlot(
		page: number,
		slot: number,
		playback: import("./types").PlaybackDefinition,
		expectedPlaybackRevision: number,
		expectedPageRevision: number,
	) {
		return this.request<{
			playback: import("./types").PlaybackDefinition;
			page: import("./types").PlaybackPage;
		}>(`/api/v1/playback-pages/${page}/slots/${slot}`, {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				playback,
				expected_playback_revision: expectedPlaybackRevision,
				expected_page_revision: expectedPageRevision,
			}),
		});
	}
	clearPlaybackSlot(
		page: number,
		slot: number,
		expectedPlaybackRevision: number,
		expectedPageRevision: number,
	) {
		return this.request<{
			cleared: boolean;
			playback_number: number;
			page: number;
			slot: number;
			page_revisions: number[];
		}>(`/api/v1/playback-pages/${page}/slots/${slot}`, {
			method: "DELETE",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				expected_playback_revision: expectedPlaybackRevision,
				expected_page_revision: expectedPageRevision,
			}),
		});
	}
	setPlaybackPage(deskId: string, page: number) {
		return this.request(`/api/v1/control-desks/${deskId}/page`, {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ page }),
		});
	}
	updateControlDesk(desk: import("./types").ControlDesk) {
		return this.request<import("./types").ControlDesk>(
			`/api/v1/control-desks/${desk.id}`,
			{
				method: "PUT",
				headers: { "content-type": "application/json" },
				body: JSON.stringify(desk),
			},
		);
	}
	removeClient(deskId: string) {
		return this.request<void>(`/api/v1/clients/${deskId}`, {
			method: "DELETE",
		});
	}

	setProgrammer(fixtureId: string, attribute: string, value: number) {
		return this.command("programmer.set", {
			fixture_id: fixtureId,
			attribute,
			value,
		});
	}
	setProgrammerMany(
		assignments: Array<{ fixtureId: string; attribute: string; value: number }>,
	) {
		return this.command("programmer.set_many", {
			assignments: assignments.map(({ fixtureId, attribute, value }) => ({
				fixture_id: fixtureId,
				attribute,
				value,
			})),
		});
	}
	setProgrammerValue(
		fixtureId: string,
		attribute: string,
		value: import("./types").AttributeValue,
	) {
		return this.command("programmer.set_value", {
			fixture_id: fixtureId,
			attribute,
			value,
		});
	}
	controlFixtureAction(fixtureId: string, actionId: string, active: boolean) {
		return this.command("programmer.control_action", {
			fixture_id: fixtureId,
			action_id: actionId,
			active,
		});
	}
	generateFixturePresets(fixtureIds: string[]) {
		return this.command("preset.generate_fixture_values", {
			fixture_ids: fixtureIds,
		}) as Promise<import("./types").GeneratedFixturePresetResult>;
	}
	releaseProgrammer(fixtureId: string, attribute: string) {
		return this.command("programmer.release", {
			fixture_id: fixtureId,
			attribute,
		});
	}
	setGroupProgrammer(
		groupId: string,
		attribute: string,
		value: number | import("./types").AttributeValue,
	) {
		return this.command("programmer.group.set", {
			group_id: groupId,
			attribute,
			value,
		});
	}
	releaseGroupProgrammer(groupId: string, attribute: string) {
		return this.command("programmer.group.release", {
			group_id: groupId,
			attribute,
		});
	}
	setGroupMaster(groupId: string, value: number) {
		return this.command("group.master.set", { group_id: groupId, value });
	}
	setGroupMasterFlash(groupId: string, value: number) {
		return this.command("group.master.flash", { group_id: groupId, value });
	}

	setSelection(fixtures: string[]) {
		return this.command("selection.set", { fixtures });
	}

	selectionGesture(
		source:
			| { type: "fixture"; fixture_id: string }
			| { type: "live_group"; group_id: string }
			| { type: "dereferenced_group"; group_id: string },
		remove = false,
	) {
		return this.command("selection.gesture", { source, remove });
	}

	setCommandLine(value: string) {
		return this.command("programmer.command_line", { value });
	}

	setCommandTarget(value: "FIXTURE" | "GROUP") {
		return this.command("programmer.command_target", { value });
	}

	executeCommandLine(value: string) {
		return this.command("programmer.execute", { value });
	}

	undoProgrammer() {
		return this.command("programmer.undo", {});
	}

	applyPreset(address: PresetAddress) {
		return this.command("preset.apply", address);
	}

	storePreset(
		showId: string,
		address: PresetAddress,
		preset: {
			name: string;
			family: PresetFamily;
			number: number;
			values: Record<string, Record<string, unknown>>;
			group_values?: Record<string, Record<string, unknown>>;
		},
		mode: "merge" | "overwrite" | "add_missing_fixtures",
		revision: number,
	) {
		const storageKey = presetStorageKey(address);
		return this.request(
			`/api/v1/shows/${showId}/presets/${encodeURIComponent(storageKey)}/store`,
			{
				method: "POST",
				headers: {
					"content-type": "application/json",
					"if-match": String(revision),
				},
				body: JSON.stringify({ mode, preset }),
			},
		);
	}

	setMaster(payload: { grand_master?: number; blackout?: boolean }) {
		return this.command("master.set", payload);
	}

	onEvent(listener: EventListener) {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	}

	connectEvents(onClose?: () => void) {
		if (!this.session)
			throw new Error("A session is required before opening events");
		this.disconnectEvents();
		const url = new URL("/api/v1/events", this.baseUrl);
		url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
		const protocols = ["light.v1", `light.token.${this.session.token}`];
		if (this.deskToken)
			protocols.push(`light.desk.b64.${this.base64Url(this.deskToken)}`);
		const socket = new WebSocket(url, protocols);
		this.socket = socket;
		socket.onclose = () => onClose?.();
		socket.addEventListener("message", (message) => {
			const data = JSON.parse(String(message.data)) as
				| ServerEvent
				| CommandResponse;
			if ("request_id" in data) {
				const pending = this.pending.get(data.request_id);
				if (!pending) return;
				window.clearTimeout(pending.timer);
				this.pending.delete(data.request_id);
				data.ok
					? pending.resolve(data.payload)
					: pending.reject(new Error(data.error ?? "Command failed"));
				return;
			}
			this.listeners.forEach((listener) => {
				listener(data);
			});
		});
		return new Promise<void>((resolve, reject) => {
			socket.addEventListener("open", () => resolve(), { once: true });
			socket.addEventListener(
				"error",
				() => reject(new Error("WebSocket connection failed")),
				{ once: true },
			);
		});
	}

	disconnectEvents() {
		if (this.socket) {
			this.socket.onclose = null;
			this.socket.close();
		}
		this.socket = null;
	}

	command(
		command: string,
		payload: unknown,
		expectedRevision?: number,
	): Promise<unknown> {
		if (
			!this.session ||
			!this.socket ||
			this.socket.readyState !== WebSocket.OPEN
		) {
			return Promise.reject(new Error("Live server connection is not ready"));
		}
		const requestId = crypto.randomUUID();
		const envelope = {
			protocol_version: 1,
			request_id: requestId,
			session_id: this.session.session_id,
			expected_revision: expectedRevision,
			command,
			payload,
		};
		return new Promise((resolve, reject) => {
			const timer = window.setTimeout(() => {
				this.pending.delete(requestId);
				reject(new Error(`Command timed out: ${command}`));
			}, 5_000);
			this.pending.set(requestId, { resolve, reject, timer });
			this.socket?.send(JSON.stringify(envelope));
		});
	}

	private async requestBlob(
		path: string,
		init: RequestInit = {},
	): Promise<Blob> {
		if (!this.session) throw new Error("A server session is required");
		const headers = this.boundaryHeaders(new Headers(init.headers));
		headers.set("authorization", `Bearer ${this.session.token}`);
		const response = await fetch(`${this.baseUrl}${path}`, {
			...init,
			headers,
		});
		if (!response.ok) throw new Error(await response.text());
		return response.blob();
	}

	private async request<T>(
		path: string,
		init: RequestInit = {},
		authenticate = true,
	): Promise<T> {
		const headers = this.boundaryHeaders(new Headers(init.headers));
		if (authenticate) {
			if (!this.session) throw new Error("A server session is required");
			headers.set("authorization", `Bearer ${this.session.token}`);
		}
		const response = await fetch(`${this.baseUrl}${path}`, {
			...init,
			headers,
		});
		if (!response.ok) {
			const body = await response.text();
			throw new Error(body || `${response.status} ${response.statusText}`);
		}
		if (response.status === 204) return undefined as T;
		return response.json() as Promise<T>;
	}
	private base64Url(value: string) {
		const bytes = new TextEncoder().encode(value);
		let binary = "";
		for (const byte of bytes) binary += String.fromCharCode(byte);
		return btoa(binary)
			.replaceAll("+", "-")
			.replaceAll("/", "_")
			.replace(/=+$/, "");
	}
}
