import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ProgrammingInteractionViewProvider } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../../features/programmingInteraction/store";
import {
	DESK_ID,
	FakeProgrammingTransport,
	FIXTURE_1,
	FIXTURE_2,
	programmingSnapshot,
	SHOW_ID,
	selectionChange,
} from "../../features/programmingInteraction/testFixtures";
import { SpecialDialogsModal } from "./SpecialDialogsModal";
import { SystemControlsModal } from "./SystemControlsModal";

const mocks = vi.hoisted(() => {
	const selectionAccess = vi.fn();
	const mutationQueue = {
		canWrite: true,
		route: "normal" as const,
		submitLatest: vi.fn(async () => undefined),
		submitBarrier: vi.fn(async () => undefined),
	};
	const server = {
		error: null,
		configuration: { programmer_fade_millis: 3_000 },
		patch: { fixtures: [] },
		playbacks: { active: [], pool: [], cue_lists: [] },
		bootstrap: { active_programmers: [] },
		session: { user: { id: "operator", name: "Operator" } },
		readVisualization: vi.fn(async () => ({
			revision: 1,
			generated_at: "2026-07-19T00:00:00Z",
			grand_master: 1,
			blackout: false,
			values: [],
		})),
		controlFixtureAction: vi.fn(async () => undefined),
		setMaster: vi.fn(async () => undefined),
		playbackAction: vi.fn(async () => undefined),
		clearProgrammer: vi.fn(async () => undefined),
		preloadAction: vi.fn(async () => undefined),
	};
	Object.defineProperty(server, "selectedFixtures", {
		get() {
			selectionAccess();
			return ["legacy-fixture"];
		},
	});
	return {
		server,
		selectionAccess,
		mutationQueue,
		mutationQueueUse: vi.fn(),
		dispatch: vi.fn(),
		appState: {
			specialDialogsOpen: false,
			systemControlsOpen: false,
			specialDialogFamily: "Dynamics" as const,
			shiftArmed: false,
		},
	};
});

vi.mock("../../api/ServerContext", () => ({ useServer: () => mocks.server }));
vi.mock(
	"../../features/programmerValues/useProgrammerValuesMutationQueue",
	async (importOriginal) => {
		const actual =
			await importOriginal<
				typeof import("../../features/programmerValues/useProgrammerValuesMutationQueue")
			>();
		return {
			...actual,
			useProgrammerValuesMutationQueue: (active: boolean) => {
				mocks.mutationQueueUse(active);
				return mocks.mutationQueue;
			},
		};
	},
);
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.appState, dispatch: mocks.dispatch }),
}));
vi.mock("../control/VerticalTouchFader", () => ({
	VerticalTouchFader: ({
		label,
		onChange,
	}: {
		label: string;
		onChange?: (value: number) => void;
	}) => (
		<button type="button" onClick={() => onChange?.(120)}>
			{label}
		</button>
	),
}));

function renderSelectionView(children: ReactNode) {
	const store = new ProgrammingInteractionStore();
	const transport = new FakeProgrammingTransport();
	const loadSnapshot = vi.fn(async () => programmingSnapshot());
	const view = (body: ReactNode) => (
		<ProgrammingInteractionViewProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			store={store}
			transport={transport}
			loadSnapshot={loadSnapshot}
		>
			{body}
		</ProgrammingInteractionViewProvider>
	);
	const rendered = render(view(children));
	return {
		...rendered,
		rerenderChildren: (body: ReactNode) => rendered.rerender(view(body)),
		transport,
		loadSnapshot,
	};
}

beforeEach(() => {
	mocks.appState.specialDialogsOpen = false;
	mocks.appState.systemControlsOpen = false;
	mocks.mutationQueue.canWrite = true;
});

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
});

describe("modal selection projections", () => {
	it("streams ordered selection into the active special dialog", async () => {
		mocks.appState.specialDialogsOpen = true;
		const { transport } = renderSelectionView(<SpecialDialogsModal />);

		await screen.findByText("1 fixtures selected");
		expect(transport.subscriptions).toHaveLength(1);
		expect(transport.subscriptions[0].scope).toEqual({
			commandLine: false,
			selection: true,
		});

		act(() =>
			transport.emit({
				type: "event",
				sequence: 20,
				correlationId: null,
				change: selectionChange({
					revision: 2,
					selected: [FIXTURE_2, FIXTURE_1],
				}),
			}),
		);

		await screen.findByText("2 fixtures selected");
		fireEvent.click(screen.getByRole("button", { name: "Dynamic speed" }));
		await waitFor(() =>
			expect(mocks.mutationQueue.submitLatest).toHaveBeenCalledOnce(),
		);
		expect(mocks.mutationQueue.submitLatest).toHaveBeenCalledWith(
			expect.any(String),
			[
				{
					action: "set_fixture",
					fixtureId: FIXTURE_2,
					attribute: "dynamic.speed",
					value: { kind: "normalized", value: 0.5 },
					timing: {
						fade: true,
						fadeMillis: 3_000,
						delayMillis: null,
					},
				},
				{
					action: "set_fixture",
					fixtureId: FIXTURE_1,
					attribute: "dynamic.speed",
					value: { kind: "normalized", value: 0.5 },
					timing: {
						fade: true,
						fadeMillis: 3_000,
						delayMillis: null,
					},
				},
			],
		);
		expect(mocks.selectionAccess).not.toHaveBeenCalled();
	});

	it("updates System Controls lamp availability from streamed selection", async () => {
		mocks.appState.systemControlsOpen = true;
		const { transport } = renderSelectionView(<SystemControlsModal />);
		const lampButton = await screen.findByRole("button", {
			name: "All Lamps On",
		});

		await waitFor(() => expect(lampButton).toBeEnabled());
		act(() =>
			transport.emit({
				type: "event",
				sequence: 20,
				correlationId: null,
				change: selectionChange({ revision: 2, selected: [] }),
			}),
		);
		await waitFor(() => expect(lampButton).toBeDisabled());

		act(() =>
			transport.emit({
				type: "event",
				sequence: 21,
				correlationId: null,
				change: selectionChange({ revision: 3, selected: [FIXTURE_2] }),
			}),
		);
		await waitFor(() => expect(lampButton).toBeEnabled());
		expect(mocks.selectionAccess).not.toHaveBeenCalled();
	});

	it("subscribes only for the lifetime of a visible modal", async () => {
		const { loadSnapshot, rerenderChildren, transport } = renderSelectionView(
			<>
				<SpecialDialogsModal />
				<SystemControlsModal />
			</>,
		);

		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
		expect(mocks.mutationQueueUse).toHaveBeenCalledWith(false);
		expect(mocks.mutationQueue.submitLatest).not.toHaveBeenCalled();
		expect(mocks.mutationQueue.submitBarrier).not.toHaveBeenCalled();
		expect(mocks.selectionAccess).not.toHaveBeenCalled();

		mocks.appState.specialDialogsOpen = true;
		rerenderChildren(
			<>
				<SpecialDialogsModal />
				<SystemControlsModal />
			</>,
		);
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		expect(loadSnapshot).toHaveBeenCalledOnce();

		mocks.appState.specialDialogsOpen = false;
		rerenderChildren(
			<>
				<SpecialDialogsModal />
				<SystemControlsModal />
			</>,
		);
		await waitFor(() =>
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce(),
		);
		expect(mocks.selectionAccess).not.toHaveBeenCalled();
	});
});
