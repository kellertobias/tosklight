import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import {
	afterEach,
	beforeEach,
	describe,
	expect,
	it,
	vi,
	type Mock,
} from "vitest";
import type { PlaybackActionRequest } from "../../api/types";
import type { PlaybackRuntimeActionApply } from "../playbackRuntime/actionWriter";
import type { PlaybackIdentity } from "../playbackRuntime/contracts";
import { PlaybackRuntimeViewProvider } from "../playbackRuntime/PlaybackRuntimeView";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import {
	cueProjection,
	DESK_ID,
	playbackSnapshot,
	SHOW_ID,
} from "../playbackRuntime/testFixtures";
import type {
	PlaybackEventObserver,
	PlaybackEventScope,
	PlaybackEventTransport,
} from "../playbackRuntime/transport";
import type { ShowObject } from "../showObjects/contracts";

const mocks = vi.hoisted(() => ({
	pagesView: {
		ready: true,
		error: null as Error | null,
		pages: [] as ShowObject<"playback_page">[],
	},
}));

vi.mock("../playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackPagesView: () => mocks.pagesView,
}));

const { DemoPlaybackControls } = await import("./DemoPlaybackControls");
const { useDemoPlaybackControls } = await import("./useDemoPlaybackControls");

function ControlsProbe({ onRender }: { onRender: () => void }) {
	onRender();
	useDemoPlaybackControls();
	return null;
}

class FakeTransport implements PlaybackEventTransport {
	readonly subscriptions: Array<{
		scope: PlaybackEventScope;
		observer: PlaybackEventObserver;
		close: ReturnType<typeof vi.fn>;
	}> = [];

	subscribe(
		_deskId: string,
		scope: PlaybackEventScope,
		_after: number | null,
		observer: PlaybackEventObserver,
	) {
		const close = vi.fn();
		this.subscriptions.push({ scope, observer, close });
		return { close, repair: vi.fn() };
	}

	/** The identities of the newest scope the surface actually subscribed. */
	get latestPlaybackNumbers() {
		const scope = this.subscriptions.at(-1)?.scope;
		return (scope?.identities ?? [])
			.flatMap((identity) =>
				identity.kind === "playback" ? [identity.playback_number] : [],
			)
			.sort((left, right) => left - right);
	}
}

function page(
	number: number,
	slots: Record<string, number>,
): ShowObject<"playback_page"> {
	return {
		kind: "playback_page",
		id: `page-${number}`,
		revision: 1,
		updated_at: "2026-07-19T10:00:00Z",
		body: { number, name: `Page ${number}`, slots },
	};
}

const DEMO_PAGE = page(1, {
	"1": 11,
	"2": 12,
	"3": 13,
	"4": 14,
	"9": 99,
	"21": 21,
	"22": 22,
	"23": 23,
	"24": 24,
});

type ApplyActionMock = Mock<PlaybackRuntimeActionApply>;

interface HarnessOptions {
	applyAction?: ApplyActionMock;
	authorityKey?: string;
}

function appliedOutcome(request: PlaybackActionRequest) {
	return {
		request_id: request.request_id,
		correlation_id: "55555555-5555-4555-8555-555555555555",
		requested: request.address,
		resolved: {
			kind: "playback" as const,
			playback_number: 11,
			page: 1,
			slot: 1,
		},
		outcome: { status: "applied" as const },
		durability: "durable" as const,
		projection: cueProjection(11),
		related: [],
		desk: null,
		event_sequence: 12,
		desk_event_sequence: null,
		replayed: false,
	};
}

function harness(options: HarnessOptions = {}) {
	const store = new PlaybackRuntimeStore();
	const transport = new FakeTransport();
	const loadSnapshot = vi.fn(async (identities: PlaybackIdentity[]) =>
		playbackSnapshot(identities),
	);
	const applyAction: ApplyActionMock =
		options.applyAction ??
		vi.fn(async (_showId, _deskId, request) => appliedOutcome(request));
	const view = (authorityKey = options.authorityKey ?? "authority-a") => (
		<PlaybackRuntimeViewProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			authorityKey={authorityKey}
			store={store}
			transport={transport}
			loadSnapshot={loadSnapshot}
			applyAction={applyAction}
		>
			<DemoPlaybackControls />
		</PlaybackRuntimeViewProvider>
	);
	return { store, transport, loadSnapshot, applyAction, view };
}

/** Narrows the recorded requests to the momentary-button sends the demo made. */
function buttonSends(applyAction: ApplyActionMock) {
	return applyAction.mock.calls.flatMap(([, , request]) =>
		request.action.type === "configured_button"
			? [
					{
						playbackNumber:
							request.address.kind === "playback"
								? request.address.playback_number
								: null,
						button: request.action.number,
						pressed: request.action.pressed,
						surface: request.surface,
					},
				]
			: [],
	);
}

function button(slot: number, index = 1) {
	return screen.getByRole("button", {
		name: `Playback ${slot} button ${index}`,
	});
}

async function settled() {
	await act(async () => {
		await new Promise<void>((resolve) => setTimeout(resolve, 0));
	});
}

beforeEach(() => {
	mocks.pagesView = { ready: true, error: null, pages: [DEMO_PAGE] };
	if (!HTMLElement.prototype.setPointerCapture)
		HTMLElement.prototype.setPointerCapture = () => undefined;
});

afterEach(cleanup);

describe("DemoPlaybackControls", () => {
	it("subscribes to exactly the Playback numbers the active desk Page maps", async () => {
		const { transport, view } = harness();
		render(view());
		await waitFor(() =>
			expect(transport.subscriptions.length).toBeGreaterThan(0),
		);
		await settled();
		// Slot 9 is mapped on the Page but is not a demo slot, so 99 stays out.
		await waitFor(() =>
			expect(transport.latestPlaybackNumbers).toEqual([
				11, 12, 13, 14, 21, 22, 23, 24,
			]),
		);
	});

	it("sends nothing while the portable Page authority is loading", async () => {
		// Pages are present but not yet authoritative: the surface must not use them.
		mocks.pagesView = { ready: false, error: null, pages: [DEMO_PAGE] };
		const { applyAction, transport, view } = harness();
		render(view());
		await settled();
		fireEvent.pointerDown(button(1), { pointerId: 1 });
		fireEvent.input(screen.getByLabelText("Playback 1 fader"), {
			target: { value: "0.5" },
		});
		await settled();
		expect(applyAction).not.toHaveBeenCalled();
		expect(transport.latestPlaybackNumbers).toEqual([]);
	});

	it("sends nothing while the authoritative desk Page is loading", async () => {
		const { applyAction, view } = harness();
		// The desk snapshot has not resolved yet, so no Page is authoritative.
		render(view());
		fireEvent.pointerDown(button(21), { pointerId: 1 });
		fireEvent.input(screen.getByLabelText("Playback 2 fader"), {
			target: { value: "0.5" },
		});
		expect(applyAction).not.toHaveBeenCalled();
		await settled();
	});

	it("renders the authoritative fader level of the mapped Playback", async () => {
		const { store, view } = harness();
		render(view());
		await settled();
		const projection = cueProjection(12);
		if (projection.target !== "cue_list" || !projection.runtime)
			throw new Error("fixture must expose a cue list runtime");
		projection.runtime.fader_position = 0.4;
		act(() => void store.applyProjection(projection, 20));
		await waitFor(() =>
			expect(screen.getByLabelText("Playback 2 fader")).toHaveValue("0.4"),
		);
		expect(screen.getByText("40%")).toBeInTheDocument();
	});

	it("sends the fader as a physical master action for the mapped Playback", async () => {
		const { applyAction, view } = harness();
		render(view());
		await settled();
		fireEvent.input(screen.getByLabelText("Playback 3 fader"), {
			target: { value: "0.75" },
		});
		await settled();
		expect(applyAction).toHaveBeenCalledOnce();
		const request = applyAction.mock.calls[0][2];
		expect(request.address).toEqual({ kind: "playback", playback_number: 13 });
		expect(request.action).toEqual({ type: "master", value: 0.75 });
		expect(request.surface).toBe("physical");
	});

	it("sends a physical press and a release for the same button", async () => {
		const { applyAction, view } = harness();
		render(view());
		await settled();
		fireEvent.pointerDown(button(1, 2), { pointerId: 1 });
		await settled();
		fireEvent.pointerUp(button(1, 2), { pointerId: 1 });
		await settled();
		expect(buttonSends(applyAction)).toEqual([
			{ playbackNumber: 11, button: 2, pressed: true, surface: "physical" },
			{ playbackNumber: 11, button: 2, pressed: false, surface: "physical" },
		]);
	});

	it("releases the originally held Playback number after the Page remaps the slot", async () => {
		const { applyAction, view } = harness();
		const rendered = render(view());
		await settled();
		fireEvent.pointerDown(button(1), { pointerId: 1 });
		await settled();

		mocks.pagesView = {
			ready: true,
			error: null,
			pages: [page(1, { ...DEMO_PAGE.body.slots, "1": 77 })],
		};
		rendered.rerender(view());
		await settled();
		fireEvent.pointerUp(button(1), { pointerId: 1 });
		await settled();

		expect(buttonSends(applyAction)).toEqual([
			{ playbackNumber: 11, button: 1, pressed: true, surface: "physical" },
			{ playbackNumber: 11, button: 1, pressed: false, surface: "physical" },
		]);
	});

	it("holds the release until the press it belongs to has settled", async () => {
		const settle: Array<() => void> = [];
		const applyAction: ApplyActionMock = vi.fn(
			async (_showId, _deskId, request) => {
				await new Promise<void>((resolve) => settle.push(resolve));
				return appliedOutcome(request);
			},
		);
		const { view } = harness({ applyAction });
		render(view());
		await settled();
		const deskLoads = settle.length;
		settle.splice(0, deskLoads).forEach((resolve) => resolve());

		fireEvent.pointerDown(button(1), { pointerId: 1 });
		await settled();
		expect(applyAction).toHaveBeenCalledOnce();

		fireEvent.pointerUp(button(1), { pointerId: 1 });
		await settled();
		expect(applyAction).toHaveBeenCalledOnce();

		settle.shift()?.();
		await settled();
		expect(applyAction).toHaveBeenCalledTimes(2);
		expect(buttonSends(applyAction).map((send) => send.pressed)).toEqual([
			true,
			false,
		]);
		settle.forEach((resolve) => resolve());
		await settled();
	});

	it.each([
		["pointer cancel", (target: HTMLElement) => fireEvent.pointerCancel(target, { pointerId: 1 })],
		["lost pointer capture", (target: HTMLElement) => fireEvent.lostPointerCapture(target, { pointerId: 1 })],
	])("releases a held button on %s", async (_name, release) => {
		const { applyAction, view } = harness();
		render(view());
		await settled();
		fireEvent.pointerDown(button(22), { pointerId: 1 });
		await settled();
		release(button(22));
		await settled();
		expect(buttonSends(applyAction)).toEqual([
			{ playbackNumber: 22, button: 1, pressed: true, surface: "physical" },
			{ playbackNumber: 22, button: 1, pressed: false, surface: "physical" },
		]);
	});

	it("releases a held button when the surface unmounts", async () => {
		const { applyAction, view } = harness();
		const rendered = render(view());
		await settled();
		fireEvent.pointerDown(button(23), { pointerId: 1 });
		await settled();
		rendered.unmount();
		await settled();
		expect(buttonSends(applyAction)).toEqual([
			{ playbackNumber: 23, button: 1, pressed: true, surface: "physical" },
			{ playbackNumber: 23, button: 1, pressed: false, surface: "physical" },
		]);
	});

	it("releases a held button when the desk authority scope is replaced", async () => {
		const { applyAction, view } = harness();
		const rendered = render(view("authority-a"));
		await settled();
		fireEvent.pointerDown(button(24), { pointerId: 1 });
		await settled();
		rendered.rerender(view("authority-b"));
		await settled();
		const releases = buttonSends(applyAction).filter((send) => !send.pressed);
		expect(releases).toEqual([
			{ playbackNumber: 24, button: 1, pressed: false, surface: "physical" },
		]);
	});

	it("releases once through the captured writer when authority stops being ready", async () => {
		const { applyAction, view } = harness();
		const rendered = render(view());
		await settled();
		fireEvent.pointerDown(button(1), { pointerId: 1 });
		await settled();

		mocks.pagesView = { ready: false, error: null, pages: [DEMO_PAGE] };
		rendered.rerender(view());
		await settled();
		fireEvent.lostPointerCapture(button(1), { pointerId: 1 });
		await settled();

		expect(buttonSends(applyAction)).toEqual([
			{ playbackNumber: 11, button: 1, pressed: true, surface: "physical" },
			{ playbackNumber: 11, button: 1, pressed: false, surface: "physical" },
		]);
		expect(button(1)).toBeDisabled();
	});

	it("does not rerender for Playback numbers the demo desk never mapped", async () => {
		const store = new PlaybackRuntimeStore();
		const onRender = vi.fn();
		render(
			<PlaybackRuntimeViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				authorityKey="authority-a"
				store={store}
				transport={new FakeTransport()}
				loadSnapshot={vi.fn(async (identities: PlaybackIdentity[]) =>
					playbackSnapshot(identities),
				)}
			>
				<ControlsProbe onRender={onRender} />
			</PlaybackRuntimeViewProvider>,
		);
		await settled();
		const settledRenders = onRender.mock.calls.length;

		// Playback 99 sits on the Page but outside the demo slots.
		act(() => void store.applyProjection(cueProjection(99), 21));
		expect(onRender).toHaveBeenCalledTimes(settledRenders);

		const mapped = cueProjection(11);
		if (mapped.target !== "cue_list" || !mapped.runtime)
			throw new Error("fixture must expose a cue list runtime");
		mapped.runtime.fader_position = 0.3;
		act(() => void store.applyProjection(mapped, 22));
		expect(onRender.mock.calls.length).toBeGreaterThan(settledRenders);
	});
});
