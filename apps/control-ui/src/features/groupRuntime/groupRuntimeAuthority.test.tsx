import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaybackRuntimeActionApply } from "../playbackRuntime/actionWriter";
import type {
	PlaybackIdentity,
	PlaybackProjection,
	PlaybackSnapshot,
} from "../playbackRuntime/contracts";
import { identityKey } from "../playbackRuntime/contracts";
import { PlaybackRuntimeViewProvider } from "../playbackRuntime/PlaybackRuntimeView";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import type {
	PlaybackEventObserver,
	PlaybackEventScope,
	PlaybackEventTransport,
} from "../playbackRuntime/transport";
import type { ShowObject, ShowObjectKind } from "../showObjects/contracts";
import { ShowObjectsViewProvider } from "../showObjects/ShowObjectsView";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventTransport,
} from "../showObjects/transport";
import {
	type RuntimeGroup,
	useGroupRuntimeAuthority,
} from "./groupRuntimeAuthority";

const SHOW_A = "11111111-1111-4111-8111-111111111111";
const SHOW_B = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const SHOW_C = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const DESK_ID = "22222222-2222-4222-8222-222222222222";

const GROUPS: Record<string, ShowObject<"group">[]> = {
	[SHOW_A]: [
		group("1", [], 0.9, null),
		group("2", ["fixture-b", "fixture-a"], 0.8, 17),
	],
	[SHOW_B]: [group("9", ["fixture-z"], 1, null)],
	[SHOW_C]: [group("a|b", [], 1, null)],
};

function group(
	id: string,
	fixtures: string[],
	master: number,
	playbackFader: number | null,
): ShowObject<"group"> {
	return {
		kind: "group",
		id,
		revision: 1,
		updated_at: "",
		body: {
			name: `Group ${id}`,
			fixtures,
			master,
			playback_fader: playbackFader,
			programming: {},
		},
	};
}

function projection(
	showId: string,
	identity: PlaybackIdentity,
	override: Partial<Extract<PlaybackProjection, { target: "group" }>> = {},
): PlaybackProjection {
	if (identity.kind !== "group")
		throw new Error(`Unexpected runtime request ${identityKey(identity)}`);
	const mapped = identity.group_id === "2" ? 17 : null;
	const master =
		identity.group_id === "1" ? 0.25 : identity.group_id === "2" ? 0.6 : 0.4;
	return {
		scope: { show_id: showId, show_revision: 4 },
		requested: identity,
		playback_number: mapped,
		target: "group",
		group_id: identity.group_id,
		master,
		flash_level: identity.group_id === "1" ? 0.1 : 0,
		...override,
	};
}

function snapshot(
	showId: string,
	identities: readonly PlaybackIdentity[],
	sequence = 10,
): PlaybackSnapshot {
	return {
		cursor: { sequence },
		desk: {
			scope: { show_id: showId, show_revision: 4 },
			desk_id: DESK_ID,
			active_page: 1,
			selected_playback: null,
		},
		projections: identities.map((identity) => projection(showId, identity)),
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

class RuntimeBackend {
	readonly requests: Array<{ showId: string; identities: PlaybackIdentity[] }> =
		[];
	readonly suspended = new Set<string>();
	private pending = new Map<
		string,
		Array<{
			identities: PlaybackIdentity[];
			resolve: (value: PlaybackSnapshot) => void;
		}>
	>();

	loader(showId: string) {
		return (identities: PlaybackIdentity[]) => this.load(showId, identities);
	}

	resolve(showId: string) {
		this.suspended.delete(showId);
		for (const pending of this.pending.get(showId) ?? [])
			pending.resolve(snapshot(showId, pending.identities));
		this.pending.delete(showId);
	}

	private load(showId: string, identities: PlaybackIdentity[]) {
		this.requests.push({ showId, identities: [...identities] });
		if (!this.suspended.has(showId))
			return Promise.resolve(snapshot(showId, identities));
		const gate = deferred<PlaybackSnapshot>();
		const pending = this.pending.get(showId) ?? [];
		pending.push({ identities: [...identities], resolve: gate.resolve });
		this.pending.set(showId, pending);
		return gate.promise;
	}
}

class ShowTransport implements ShowObjectsEventTransport {
	readonly scopes: ShowObjectsEventScope[] = [];

	subscribe(
		_showId: string,
		scope: ShowObjectsEventScope,
		_afterSequence: number | null,
		_observer: ShowObjectsEventObserver,
	) {
		this.scopes.push(scope);
		return { close: vi.fn(), repair: vi.fn() };
	}
}

class RuntimeTransport implements PlaybackEventTransport {
	readonly scopes: PlaybackEventScope[] = [];
	readonly repairs: ReturnType<typeof vi.fn>[] = [];
	observer: PlaybackEventObserver | null = null;

	subscribe(
		_deskId: string,
		scope: PlaybackEventScope,
		_afterSequence: number | null,
		observer: PlaybackEventObserver,
	) {
		this.scopes.push(scope);
		this.observer = observer;
		const repair = vi.fn();
		this.repairs.push(repair);
		return { close: vi.fn(), repair };
	}
}

let latestGroups: readonly RuntimeGroup[] = [];

function Probe({
	enabled,
	onRender,
}: {
	enabled: boolean;
	onRender: () => void;
}) {
	onRender();
	const authority = useGroupRuntimeAuthority(enabled);
	latestGroups = authority.groups;
	return (
		<div>
			<span data-testid="ready">{String(authority.ready)}</span>
			<span data-testid="can-write">{String(authority.canWrite)}</span>
			<span data-testid="groups">
				{authority.groups
					.map(
						(group) =>
							`${group.id}:${group.body.fixtures.join(",")}:${group.runtime.master}:${group.runtime.flashLevel}:${group.runtime.playbackNumber ?? "direct"}`,
					)
					.join("|")}
			</span>
		</div>
	);
}

function harness(
	options: {
		enabled?: boolean;
		showId?: string;
		runtime?: RuntimeBackend;
		applyAction?: PlaybackRuntimeActionApply | null;
	} = {},
) {
	const runtime = options.runtime ?? new RuntimeBackend();
	const showStore = new ShowObjectsStore();
	const runtimeStore = new PlaybackRuntimeStore();
	const showTransport = new ShowTransport();
	const runtimeTransport = new RuntimeTransport();
	const onRender = vi.fn();
	const loadCollection = vi.fn(
		async (showId: string, kind: ShowObjectKind) => ({
			objects: kind === "group" ? GROUPS[showId] : [],
			showRevision: 4,
		}),
	);
	const tree = (
		showId = options.showId ?? SHOW_A,
		enabled = options.enabled ?? true,
	) => (
		<ShowObjectsViewProvider
			showId={showId}
			authorityKey={`authority-${showId}`}
			store={showStore}
			transport={showTransport}
			loadCollection={loadCollection}
			loadObject={vi.fn()}
		>
			<PlaybackRuntimeViewProvider
				showId={showId}
				deskId={DESK_ID}
				authorityKey={`authority-${showId}`}
				store={runtimeStore}
				transport={runtimeTransport}
				loadSnapshot={runtime.loader(showId)}
				applyAction={options.applyAction}
			>
				<Probe enabled={enabled} onRender={onRender} />
			</PlaybackRuntimeViewProvider>
		</ShowObjectsViewProvider>
	);
	const view = render(tree());
	return {
		loadCollection,
		onRender,
		runtime,
		runtimeStore,
		runtimeTransport,
		showTransport,
		showStore,
		view,
		rerender: (showId: string, enabled = true) =>
			view.rerender(tree(showId, enabled)),
	};
}

afterEach(() => {
	latestGroups = [];
	cleanup();
});

describe("Group runtime authority", () => {
	it("opens no portable/runtime snapshot or socket while dormant", async () => {
		const model = harness({ enabled: false });
		await act(async () => undefined);

		expect(model.loadCollection).not.toHaveBeenCalled();
		expect(model.showTransport.scopes).toHaveLength(0);
		expect(model.runtime.requests).toHaveLength(0);
		expect(model.runtimeTransport.scopes).toHaveLength(0);
		expect(screen.getByTestId("ready")).toHaveTextContent("false");
	});

	it("hydrates exact Groups, preserving stored-empty order and direct/mapped identity", async () => {
		const model = harness();
		await waitFor(() =>
			expect(screen.getByTestId("ready")).toHaveTextContent("true"),
		);

		expect(model.loadCollection).toHaveBeenCalledWith(SHOW_A, "group");
		expect(model.showTransport.scopes.at(-1)).toEqual({
			kinds: ["group"],
			objects: [],
		});
		const requested = new Set(
			model.runtime.requests.flatMap(({ identities }) =>
				identities.map(identityKey),
			),
		);
		expect(requested).toEqual(new Set(["group:1", "group:2"]));
		expect(model.runtimeTransport.scopes.at(-1)).toEqual({
			identities: [
				{ kind: "group", group_id: "1" },
				{ kind: "group", group_id: "2" },
			],
			desk: false,
		});
		expect(screen.getByTestId("groups")).toHaveTextContent(
			"1::0.25:0.1:direct|2:fixture-b,fixture-a:0.6:0:17",
		);
		expect(latestGroups[0].body.fixtures).toEqual([]);
		expect(latestGroups[1].body.fixtures).toEqual(["fixture-b", "fixture-a"]);
		expect(screen.getByTestId("can-write")).toHaveTextContent("false");
	});

	it("advertises writes only when the real runtime writer is mounted", async () => {
		harness({ applyAction: vi.fn() });
		await waitFor(() =>
			expect(screen.getByTestId("ready")).toHaveTextContent("true"),
		);
		expect(screen.getByTestId("can-write")).toHaveTextContent("true");
	});

	it("exposes no stale portable master while exact runtime is loading", async () => {
		const runtime = new RuntimeBackend();
		runtime.suspended.add(SHOW_A);
		const model = harness({ runtime });
		await waitFor(() => expect(runtime.requests.length).toBeGreaterThan(0));

		expect(screen.getByTestId("ready")).toHaveTextContent("false");
		expect(screen.getByTestId("groups")).toBeEmptyDOMElement();
		expect(screen.queryByText("0.9")).toBeNull();

		act(() => runtime.resolve(SHOW_A));
		await waitFor(() =>
			expect(screen.getByTestId("ready")).toHaveTextContent("true"),
		);
		expect(model.runtimeTransport.scopes).toHaveLength(1);
	});

	it("drops the prior Show immediately during authority replacement", async () => {
		const runtime = new RuntimeBackend();
		const model = harness({ runtime });
		await waitFor(() =>
			expect(screen.getByTestId("groups")).toHaveTextContent("1::0.25"),
		);
		runtime.suspended.add(SHOW_B);

		model.rerender(SHOW_B);

		expect(screen.getByTestId("ready")).toHaveTextContent("false");
		expect(screen.getByTestId("groups")).toBeEmptyDOMElement();
		await waitFor(() =>
			expect(runtime.requests.some(({ showId }) => showId === SHOW_B)).toBe(
				true,
			),
		);
		act(() => runtime.resolve(SHOW_B));
		await waitFor(() =>
			expect(screen.getByTestId("groups")).toHaveTextContent(
				"9:fixture-z:0.4:0:direct",
			),
		);
		expect(screen.getByTestId("groups")).not.toHaveTextContent("1::0.25");
	});

	it("reuses unaffected Group objects and ignores unrelated runtime projections", async () => {
		const model = harness();
		await waitFor(() =>
			expect(screen.getByTestId("ready")).toHaveTextContent("true"),
		);
		const first = latestGroups[0];
		const second = latestGroups[1];

		act(() => {
			model.runtimeStore.applyProjection(
				projection(SHOW_A, { kind: "group", group_id: "1" }, { master: 0.5 }),
				20,
			);
		});
		expect(latestGroups[0]).not.toBe(first);
		expect(latestGroups[1]).toBe(second);
		const renders = model.onRender.mock.calls.length;

		act(() => {
			model.runtimeStore.applyProjection(
				projection(SHOW_A, { kind: "group", group_id: "99" }),
				21,
			);
		});
		expect(model.onRender).toHaveBeenCalledTimes(renders);
		expect(latestGroups[1]).toBe(second);
	});

	it("repairs a cursor gap with only the exact Group identities", async () => {
		const model = harness();
		await waitFor(() => expect(model.runtimeTransport.observer).not.toBeNull());
		const requests = model.runtime.requests.length;

		act(() => {
			model.runtimeTransport.observer?.message({
				type: "gap",
				afterSequence: 10,
				oldestAvailable: 11,
				latestSequence: 20,
			});
		});

		await waitFor(() =>
			expect(model.runtime.requests.length).toBeGreaterThan(requests),
		);
		expect(
			new Set(model.runtime.requests.at(-1)?.identities.map(identityKey)),
		).toEqual(new Set(["group:1", "group:2"]));
		expect(model.runtimeTransport.repairs.at(-1)).toHaveBeenCalledWith(10);
	});

	it("distinguishes one opaque Group containing a delimiter from two Groups", async () => {
		const model = harness({ showId: SHOW_C });
		await waitFor(() =>
			expect(screen.getByTestId("groups")).toHaveTextContent("a|b::0.4"),
		);
		const requests = model.runtime.requests.length;

		act(() => {
			model.showStore.setCollection(
				SHOW_C,
				"group",
				[group("a", [], 1, null), group("b", [], 1, null)],
				5,
			);
		});

		await waitFor(() =>
			expect(model.runtime.requests.length).toBeGreaterThan(requests),
		);
		expect(
			new Set(model.runtime.requests.at(-1)?.identities.map(identityKey)),
		).toEqual(new Set(["group:a", "group:b"]));
		await waitFor(() =>
			expect(screen.getByTestId("groups")).toHaveTextContent(
				"a::0.4:0:direct|b::0.4:0:direct",
			),
		);
	});
});
