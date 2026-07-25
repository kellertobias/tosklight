import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CueList, PlaybackActionRequest, PlaybackDefinition } from "../../../api/types";
import type { PlaybackRuntimeActionApply } from "../../../features/playbackRuntime/actionWriter";
import type {
	PlaybackIdentity,
	PlaybackProjection,
	PlaybackSnapshot,
} from "../../../features/playbackRuntime/contracts";
import { identityKey } from "../../../features/playbackRuntime/contracts";
import { PlaybackRuntimeViewProvider } from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { PlaybackRuntimeStore } from "../../../features/playbackRuntime/store";
import { cueProjection, deskProjection } from "../../../features/playbackRuntime/testFixtures";
import type {
	PlaybackEventObserver,
	PlaybackEventScope,
	PlaybackEventTransport,
} from "../../../features/playbackRuntime/transport";
import type {
	ShowObject,
	ShowObjectKind,
} from "../../../features/showObjects/contracts";
import { ShowObjectsStore } from "../../../features/showObjects/store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventTransport,
} from "../../../features/showObjects/transport";
import { ShowObjectsViewProvider } from "../../../features/showObjects/ShowObjectsView";
import {
	type RunningPlaybackAuthority,
	useRunningPlaybackAuthority,
} from "./runningPlaybackAuthority";

const SHOW_A = "11111111-1111-4111-8111-111111111111";
const SHOW_B = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const DESK_ID = "22222222-2222-4222-8222-222222222222";

function cueList(id: string, name: string, dynamics = 0): CueList {
	return {
		id,
		name,
		priority: 10,
		mode: "sequence",
		looped: false,
		cues: [
			{
				id: `${id}-cue-1`,
				number: 1,
				name: "Opening",
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: [],
				phasers: Array.from({ length: dynamics }, () => ({})),
			},
		],
	};
}

function playback(
	number: number,
	name: string,
	target: PlaybackDefinition["target"],
): PlaybackDefinition {
	return {
		number,
		name,
		target,
		buttons: ["go", "go_minus", "flash"],
		fader: "master",
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
	};
}

function showObject<K extends "cue_list" | "playback">(
	kind: K,
	id: string,
	body: K extends "cue_list" ? CueList : PlaybackDefinition,
): ShowObject<K> {
	return { kind, id, revision: 1, updated_at: "", body } as ShowObject<K>;
}

const OBJECTS = {
	[SHOW_A]: {
		cue_list: [
			showObject("cue_list", "main", cueList("main", "Main Cuelist", 2)),
			showObject("cue_list", "virtual", cueList("virtual", "Virtual Cuelist")),
		],
		playback: [
			showObject(
				"playback",
				"12",
				playback(12, "Main playback", {
					type: "cue_list",
					cue_list_id: "main",
				}),
			),
			showObject(
				"playback",
				"99",
				playback(99, "Group master", { type: "group", group_id: "front" }),
			),
		],
	},
	[SHOW_B]: {
		cue_list: [
			showObject(
				"cue_list",
				"replacement",
				cueList("replacement", "Replacement Cuelist"),
			),
		],
		playback: [
			showObject(
				"playback",
				"7",
				playback(7, "Replacement playback", {
					type: "cue_list",
					cue_list_id: "replacement",
				}),
			),
		],
	},
};

class ShowTransport implements ShowObjectsEventTransport {
	readonly subscriptions: ShowObjectsEventScope[] = [];
	readonly closes: ReturnType<typeof vi.fn>[] = [];

	subscribe(
		_showId: string,
		scope: ShowObjectsEventScope,
		_afterSequence: number | null,
		_observer: ShowObjectsEventObserver,
	) {
		this.subscriptions.push(scope);
		const close = vi.fn();
		this.closes.push(close);
		return { close, repair: vi.fn() };
	}
}

class RuntimeTransport implements PlaybackEventTransport {
	readonly subscriptions: PlaybackEventScope[] = [];
	readonly closes: ReturnType<typeof vi.fn>[] = [];

	subscribe(
		_deskId: string,
		scope: PlaybackEventScope,
		_afterSequence: number | null,
		_observer: PlaybackEventObserver,
	) {
		this.subscriptions.push(scope);
		const close = vi.fn();
		this.closes.push(close);
		return { close, repair: vi.fn() };
	}
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

class RuntimeBackend {
	readonly requests: Array<{ showId: string; identities: PlaybackIdentity[] }> = [];
	readonly suspended = new Set<string>();
	private pending = new Map<
		string,
		Array<{
			identities: PlaybackIdentity[];
			resolve: (snapshot: PlaybackSnapshot) => void;
		}>
	>();

	loader(showId: string) {
		return (identities: PlaybackIdentity[]) => this.load(showId, identities);
	}

	resolve(showId: string) {
		this.suspended.delete(showId);
		for (const run of this.pending.get(showId) ?? [])
			run.resolve(snapshot(showId, run.identities));
		this.pending.delete(showId);
	}

	private load(showId: string, identities: PlaybackIdentity[]) {
		this.requests.push({ showId, identities: [...identities] });
		if (!this.suspended.has(showId))
			return Promise.resolve(snapshot(showId, identities));
		const gate = deferred<PlaybackSnapshot>();
		const runs = this.pending.get(showId) ?? [];
		runs.push({ identities: [...identities], resolve: gate.resolve });
		this.pending.set(showId, runs);
		return gate.promise;
	}
}

function snapshot(
	showId: string,
	identities: readonly PlaybackIdentity[],
): PlaybackSnapshot {
	return {
		cursor: { sequence: 10 },
		desk: {
			...deskProjection(),
			scope: { show_id: showId, show_revision: 4 },
			desk_id: DESK_ID,
		},
		projections: identities.map((identity) => projection(showId, identity)),
	};
}

function projection(
	showId: string,
	requested: PlaybackIdentity,
): PlaybackProjection {
	const cueListId =
		requested.kind === "playback"
			? requested.playback_number === 12
				? "main"
				: "replacement"
			: requested.kind === "cue_list"
				? requested.cue_list_id
				: unexpectedGroupIdentity(requested.group_id);
	const playbackNumber =
		requested.kind === "playback"
			? requested.playback_number
			: cueListId === "main"
				? 12
				: cueListId === "replacement"
					? 7
					: null;
	const base = cueProjection(playbackNumber ?? 1);
	if (base.target !== "cue_list" || !base.runtime)
		throw new Error("shared fixture must contain a running Cuelist");
	return {
		...base,
		scope: { show_id: showId, show_revision: 4 },
		requested,
		playback_number: playbackNumber,
		cue_list_id: cueListId,
		runtime: { ...base.runtime, master: playbackNumber === 12 ? 0.75 : 1 },
	};
}

function unexpectedGroupIdentity(groupId: string): never {
	throw new Error(`System Controls requested unrelated Group runtime ${groupId}`);
}

let latest: RunningPlaybackAuthority | null = null;

function Probe({ enabled, rendered }: { enabled: boolean; rendered?: () => void }) {
	const authority = useRunningPlaybackAuthority(enabled);
	latest = authority;
	rendered?.();
	return (
		<div>
			<span data-testid="ready">{String(authority.ready)}</span>
			<span data-testid="sources">
				{authority.sources.map((source) => source.label).join("|")}
			</span>
			<span data-testid="dynamics">{authority.dynamics.length}</span>
			<button
				type="button"
				onClick={() =>
					void authority.release({
						identity: { kind: "playback", playback_number: 12 },
						cueListId: "main",
					})
				}
			>
				Release known source
			</button>
		</div>
	);
}

function harness(options: {
	enabled?: boolean;
	showId?: string;
	runtime?: RuntimeBackend;
	applyAction?: PlaybackRuntimeActionApply;
	rendered?: () => void;
} = {}) {
	const showStore = new ShowObjectsStore();
	const runtimeStore = new PlaybackRuntimeStore();
	const showTransport = new ShowTransport();
	const runtimeTransport = new RuntimeTransport();
	const runtime = options.runtime ?? new RuntimeBackend();
	const loadCollection = vi.fn(
		async (showId: string, kind: ShowObjectKind) => ({
			objects:
				kind === "cue_list" || kind === "playback"
					? OBJECTS[showId as keyof typeof OBJECTS][kind]
					: [],
			showRevision: 4,
		}),
	);
	const applyAction = options.applyAction ?? vi.fn();
	const tree = (
		showId = options.showId ?? SHOW_A,
		enabled = options.enabled ?? true,
		authorityKey = `authority-${showId}`,
	) => (
		<ShowObjectsViewProvider
			showId={showId}
			authorityKey={authorityKey}
			store={showStore}
			transport={showTransport}
			loadCollection={loadCollection}
			loadObject={vi.fn()}
		>
			<PlaybackRuntimeViewProvider
				showId={showId}
				deskId={DESK_ID}
				authorityKey={authorityKey}
				store={runtimeStore}
				transport={runtimeTransport}
				loadSnapshot={runtime.loader(showId)}
				applyAction={applyAction}
			>
				<Probe enabled={enabled} rendered={options.rendered} />
			</PlaybackRuntimeViewProvider>
		</ShowObjectsViewProvider>
	);
	const view = render(tree());
	return {
		applyAction,
		loadCollection,
		runtime,
		runtimeStore,
		runtimeTransport,
		showTransport,
		showStore,
		view,
		rerender: (showId: string, enabled = true, authorityKey?: string) =>
			view.rerender(tree(showId, enabled, authorityKey)),
	};
}

afterEach(() => {
	latest = null;
	cleanup();
});

describe("System Controls running Playback authority", () => {
	it("opens no portable or runtime snapshot and no socket while disabled", async () => {
		const model = harness({ enabled: false });

		await Promise.resolve();

		expect(model.loadCollection).not.toHaveBeenCalled();
		expect(model.showTransport.subscriptions).toHaveLength(0);
		expect(model.runtime.requests).toHaveLength(0);
		expect(model.runtimeTransport.subscriptions).toHaveLength(0);
		expect(screen.getByTestId("ready")).toHaveTextContent("false");
	});

	it("hydrates only Cuelists and Playbacks, then selects exact running identities", async () => {
		const model = harness();

		await waitFor(() =>
			expect(screen.getByTestId("ready")).toHaveTextContent("true"),
		);

		expect(
			model.loadCollection.mock.calls.map(([, kind]) => kind).sort(),
		).toEqual(["cue_list", "playback"]);
		expect(model.showTransport.subscriptions.at(-1)).toEqual({
			kinds: ["cue_list", "playback"],
			objects: [],
		});
		const requested = new Set(
			model.runtime.requests.flatMap(({ identities }) => identities.map(identityKey)),
		);
		expect(requested).toEqual(
			new Set(["playback:12", "cuelist:main", "cuelist:virtual"]),
		);
		expect(model.runtimeTransport.subscriptions.at(-1)).toMatchObject({
			desk: false,
		});
		expect(
			new Set(
				model.runtimeTransport.subscriptions
					.at(-1)
					?.identities.map(identityKey),
			),
		).toEqual(requested);
		expect(screen.getByTestId("sources")).toHaveTextContent(
			"Main playback|Virtual Cuelist",
		);
		expect(screen.getByTestId("dynamics")).toHaveTextContent("2");
	});

	it("refuses release and exposes no stale rows while exact runtime is loading", async () => {
		const runtime = new RuntimeBackend();
		runtime.suspended.add(SHOW_A);
		const applyAction = vi.fn();
		const model = harness({ runtime, applyAction });
		await waitFor(() => expect(runtime.requests.length).toBeGreaterThan(0));

		expect(screen.getByTestId("ready")).toHaveTextContent("false");
		expect(screen.getByTestId("sources")).toBeEmptyDOMElement();
		fireEvent.click(screen.getByRole("button", { name: "Release known source" }));

		expect(applyAction).not.toHaveBeenCalled();
		expect(await latest?.release({
			identity: { kind: "playback", playback_number: 12 },
			cueListId: "main",
		})).toBeNull();
		expect(model.runtimeTransport.subscriptions).toHaveLength(0);
	});

	it("drops prior Show rows until replacement authority is hydrated", async () => {
		const runtime = new RuntimeBackend();
		const model = harness({ runtime });
		await waitFor(() =>
			expect(screen.getByTestId("sources")).toHaveTextContent("Main playback"),
		);
		runtime.suspended.add(SHOW_B);

		model.rerender(SHOW_B);

		expect(screen.getByTestId("ready")).toHaveTextContent("false");
		expect(screen.getByTestId("sources")).toBeEmptyDOMElement();
		await waitFor(() =>
			expect(screen.getByTestId("ready")).toHaveTextContent("false"),
		);
		expect(screen.getByTestId("sources")).toBeEmptyDOMElement();
		expect(screen.queryByText("Main playback")).not.toBeInTheDocument();
		await waitFor(() =>
			expect(
				runtime.requests.some(({ showId }) => showId === SHOW_B),
			).toBe(true),
		);

		act(() => runtime.resolve(SHOW_B));

		await waitFor(() =>
			expect(model.runtimeStore.getSnapshot().status).toBe("ready"),
		);
		expect(model.showStore.getSnapshot().readyCollections).toEqual(
			new Set(["cue_list", "playback"]),
		);
		expect(model.showStore.getSnapshot().playbacks.map(({ id }) => id)).toEqual([
			"7",
		]);
		expect(model.showStore.getSnapshot().cueLists.map(({ id }) => id)).toEqual([
			"replacement",
		]);
		expect([...model.runtimeStore.getSnapshot().projections.keys()].sort()).toEqual(
			["cuelist:replacement", "playback:7"],
		);
		expect(
			model.runtimeStore
				.getSnapshot()
				.projections.get("cuelist:replacement")?.length,
		).toBeGreaterThan(0);
		await waitFor(() =>
			expect(screen.getByTestId("sources")).toHaveTextContent(
				"Replacement playback",
			),
		);
		expect(screen.getByTestId("sources")).not.toHaveTextContent("Main playback");
	});

	it("does not rerender for an unrelated runtime projection", async () => {
		const rendered = vi.fn();
		const model = harness({ rendered });
		await waitFor(() =>
			expect(screen.getByTestId("ready")).toHaveTextContent("true"),
		);
		const before = rendered.mock.calls.length;

		act(() => {
			model.runtimeStore.applyProjection(
				{
					scope: { show_id: SHOW_A, show_revision: 5 },
					requested: { kind: "playback", playback_number: 88 },
					playback_number: 88,
					target: "missing",
				},
				20,
			);
		});

		expect(rendered).toHaveBeenCalledTimes(before);
	});
});
