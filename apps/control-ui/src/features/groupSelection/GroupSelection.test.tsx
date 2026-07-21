import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { GroupStrip } from "../../components/shared/GroupStrip";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import { GroupPoolGrid } from "../../windows/groupsWindow/GroupPoolGrid";
import type {
	ProgrammingSnapshot,
	SelectionActionOutcome,
	SelectionActionRequest,
	SelectionProjection,
} from "../programmingInteraction/contracts";
import { ProgrammingInteractionViewProvider } from "../programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../programmingInteraction/store";
import {
	DESK_ID,
	FakeProgrammingTransport,
	OTHER_DESK_ID,
	OTHER_SHOW_ID,
	commandLine,
	programmingSnapshot,
	SHOW_ID,
	selection,
	selectionChange,
} from "../programmingInteraction/testFixtures";
import type { ShowObject } from "../showObjects/contracts";
import { ShowObjectsStateProvider } from "../showObjects/ShowObjectsState";
import { ShowObjectsStore } from "../showObjects/store";

const SHOW_REVISION = 7;
/** Deliberately unsorted so an ordered membership cannot pass by coincidence. */
const ORDERED_MEMBERS = ["fixture-3", "fixture-1", "fixture-2"];

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	replaceCommandLine: vi.fn(),
	recordGroup: vi.fn(),
	playbackReads: 0,
	state: { storeArmed: false, updateArmed: false },
}));

vi.mock("../../api/ServerContext", () => ({
	useServer: () => ({
		bootstrap: { active_show: { id: SHOW_ID } },
		get playbacks() {
			mocks.playbackReads += 1;
			return null;
		},
	}),
}));
vi.mock("../groupRecording/GroupRecordingProvider", () => ({
	useGroupRecording: () => ({ record: mocks.recordGroup }),
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));

function group(
	id: string,
	name: string,
	fixtures: readonly string[] = ORDERED_MEMBERS,
): ShowObject<"group"> {
	return {
		kind: "group",
		id,
		revision: 3,
		updated_at: "",
		body: { name, fixtures: [...fixtures], master: 1 },
	};
}

function groupOutcome(
	request: SelectionActionRequest,
	projection: SelectionProjection,
	action: SelectionActionOutcome["action"] = "gesture_applied",
	replayed = false,
): SelectionActionOutcome {
	return {
		requestId: request.requestId,
		correlationId: request.requestId,
		action,
		applied: projection.selected.length,
		selection: projection,
		eventSequence: 11,
		replayed,
		warning: null,
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((onResolve, onReject) => {
		resolve = onResolve;
		reject = onReject;
	});
	return { promise, resolve, reject };
}

/** The Group Pool grid as GroupsWindow composes it, minus unrelated pool chrome. */
function GroupPool({
	active = true,
	cards,
}: {
	active?: boolean;
	cards: (ShowObject<"group"> | null)[];
}) {
	const command = useCommandLineSurface({
		selection: true,
		enabled: active,
		observeCommand: false,
	});
	return (
		<GroupPoolGrid
			active={active}
			cards={cards}
			capabilities={new Map()}
			knownFixtureIds={new Set()}
			command={command}
			onOpenContext={() => undefined}
			onOpenProperties={() => undefined}
			onOpenRecord={() => undefined}
			recordGroup={async () => null}
			runCommand={async () => null}
		/>
	);
}

type ApplySelection = (
	deskId: string,
	request: SelectionActionRequest,
) => Promise<SelectionActionOutcome>;
type LoadSnapshot = () => Promise<ProgrammingSnapshot>;

interface Harness {
	showObjects: ShowObjectsStore;
	programming: ProgrammingInteractionStore;
	transport: FakeProgrammingTransport;
	applySelection: Mock<ApplySelection>;
	loadSnapshot: Mock<LoadSnapshot>;
}

function harness(): Harness {
	return {
		showObjects: new ShowObjectsStore(),
		programming: new ProgrammingInteractionStore(),
		transport: new FakeProgrammingTransport(),
		applySelection: vi.fn<ApplySelection>(),
		loadSnapshot: vi.fn<LoadSnapshot>(async () => programmingSnapshot()),
	};
}

function view(
	context: Harness,
	children: React.ReactNode,
	showId: string = SHOW_ID,
	withActions = true,
) {
	return (
		<ShowObjectsStateProvider store={context.showObjects}>
			<ProgrammingInteractionViewProvider
				showId={showId}
				deskId={DESK_ID}
				store={context.programming}
				transport={context.transport}
				loadSnapshot={context.loadSnapshot}
				replaceCommandLine={mocks.replaceCommandLine}
				applySelection={withActions ? context.applySelection : undefined}
			>
				{children}
			</ProgrammingInteractionViewProvider>
		</ShowObjectsStateProvider>
	);
}

function installGroups(
	context: Harness,
	groups: ShowObject<"group">[],
	showRevision = SHOW_REVISION,
	showId = SHOW_ID,
) {
	act(() =>
		context.showObjects.setCollection(
			showId,
			"group",
			groups,
			undefined,
			showRevision,
		),
	);
}

async function settleAuthority(context: Harness) {
	await waitFor(() =>
		expect(context.programming.getSnapshot().selection).not.toBeNull(),
	);
}

function requestOf(applySelection: Mock<ApplySelection>, index = 0) {
	return applySelection.mock.calls[index]?.[1] as SelectionActionRequest;
}

describe("scoped Group activation", () => {
	afterEach(() => cleanup());

	beforeEach(() => {
		mocks.dispatch.mockReset();
		mocks.replaceCommandLine.mockReset().mockResolvedValue(commandLine(2, "GROUP 1", "GROUP"));
		mocks.recordGroup.mockReset().mockResolvedValue({ status: "changed" });
		mocks.playbackReads = 0;
		mocks.state.storeArmed = false;
		mocks.state.updateArmed = false;
	});

	it("sends exactly one live gesture with the ordered resolved membership", async () => {
		const context = harness();
		const answer = deferred<SelectionActionOutcome>();
		context.applySelection.mockReturnValue(answer.promise);
		render(view(context, <GroupStrip />));
		installGroups(context, [group("1", "Front Truss")]);
		await settleAuthority(context);

		fireEvent.click(screen.getByText("Front Truss").closest("button")!);

		await waitFor(() => expect(context.applySelection).toHaveBeenCalledOnce());
		expect(requestOf(context.applySelection).action).toEqual({
			type: "gesture",
			source: { type: "live_group", groupId: "1" },
			remove: false,
		});
		expect(context.programming.getSnapshot().selection?.selected).toEqual(
			ORDERED_MEMBERS,
		);
		expect(mocks.replaceCommandLine).toHaveBeenCalledWith(
			DESK_ID,
			"GROUP 1",
			expect.any(Number),
		);
	});

	it("sends exactly one frozen selectGroup action with the captured Show revision", async () => {
		const context = harness();
		const answer = deferred<SelectionActionOutcome>();
		context.applySelection.mockReturnValue(answer.promise);
		render(view(context, <GroupStrip />));
		installGroups(context, [group("1", "Front Truss")]);
		await settleAuthority(context);

		fireEvent.doubleClick(screen.getByText("Front Truss").closest("button")!);
		await waitFor(() => expect(context.applySelection).toHaveBeenCalledOnce());
		// A later Show revision must not retarget the capture already in flight.
		installGroups(context, [group("1", "Front Truss")], 9);

		expect(requestOf(context.applySelection).action).toEqual({
			type: "select_group",
			groupId: "1",
			frozen: true,
			rule: { type: "all" },
			expectedRevision: 1,
		});
		expect(context.programming.getSnapshot().selection?.expression).toEqual({
			type: "frozen_group",
			groupId: "1",
			sourceRevision: SHOW_REVISION,
		});
		expect(context.programming.getSnapshot().selection?.selected).toEqual(
			ORDERED_MEMBERS,
		);
	});

	it("keeps a stored empty Group selectable", async () => {
		const context = harness();
		context.applySelection.mockImplementation(
			async (_desk, request) =>
				groupOutcome(request, selection(2, [])),
		);
		render(view(context, <GroupStrip />));
		installGroups(context, [group("1", "Stored Empty", [])]);
		await settleAuthority(context);

		fireEvent.click(screen.getByText("Stored Empty").closest("button")!);

		await waitFor(() => expect(context.applySelection).toHaveBeenCalledOnce());
		expect(requestOf(context.applySelection).action).toMatchObject({
			type: "gesture",
			source: { type: "live_group", groupId: "1" },
		});
		expect(mocks.replaceCommandLine).toHaveBeenCalledWith(
			DESK_ID,
			"GROUP 1",
			expect.any(Number),
		);
	});

	it("sends nothing while Group authority is still loading", async () => {
		const context = harness();
		render(view(context, <GroupStrip />));
		await settleAuthority(context);

		// Mock-data slots render before any authoritative Group collection arrives.
		const card = screen.getAllByRole("button")[0];
		fireEvent.click(card);
		fireEvent.doubleClick(card);

		expect(context.applySelection).not.toHaveBeenCalled();
		expect(mocks.replaceCommandLine).not.toHaveBeenCalled();
	});

	it("sends nothing while scoped Programming selection actions are absent", async () => {
		const context = harness();
		render(view(context, <GroupStrip />, SHOW_ID, false));
		installGroups(context, [group("1", "Front Truss")]);
		await settleAuthority(context);

		fireEvent.click(screen.getByText("Front Truss").closest("button")!);
		fireEvent.doubleClick(screen.getByText("Front Truss").closest("button")!);

		expect(context.applySelection).not.toHaveBeenCalled();
		expect(mocks.replaceCommandLine).not.toHaveBeenCalled();
	});

	it("rolls the optimistic selection back when the action is rejected", async () => {
		const context = harness();
		const rejection = Object.assign(new Error("rejected"), { status: 400 });
		context.applySelection.mockRejectedValue(rejection);
		render(view(context, <GroupStrip />));
		installGroups(context, [group("1", "Front Truss")]);
		await settleAuthority(context);
		const before = context.programming.getSnapshot().selection;

		fireEvent.click(screen.getByText("Front Truss").closest("button")!);

		await waitFor(() => expect(context.applySelection).toHaveBeenCalledOnce());
		await waitFor(() =>
			expect(context.programming.getSnapshot().selection).toEqual(before),
		);
	});

	it("settles a replayed no-change outcome without disturbing authority", async () => {
		const context = harness();
		// A replay reports the current authority again at its own revision.
		const unchanged = selection(2, ORDERED_MEMBERS);
		context.applySelection.mockImplementation(
			async (_desk, request) =>
				groupOutcome(request, unchanged, "gesture_applied", true),
		);
		render(view(context, <GroupStrip />));
		installGroups(context, [group("1", "Front Truss")]);
		await settleAuthority(context);

		fireEvent.click(screen.getByText("Front Truss").closest("button")!);

		await waitFor(() =>
			expect(context.programming.getSnapshot().selection).toEqual(unchanged),
		);
		expect(context.programming.getSnapshot().error).toBeNull();
	});

	it("keeps a late response out of a replacement Show scope", async () => {
		const context = harness();
		const answer = deferred<SelectionActionOutcome>();
		context.applySelection.mockReturnValue(answer.promise);
		context.loadSnapshot
			.mockResolvedValueOnce(programmingSnapshot())
			.mockResolvedValueOnce(
				programmingSnapshot({ sequence: 20, selected: selection(5, []) }),
			);
		const rendered = render(view(context, <GroupStrip />));
		installGroups(context, [group("1", "Front Truss")]);
		await settleAuthority(context);
		fireEvent.click(screen.getByText("Front Truss").closest("button")!);
		await waitFor(() => expect(context.applySelection).toHaveBeenCalledOnce());

		rendered.rerender(view(context, <GroupStrip />, OTHER_SHOW_ID));
		await waitFor(() =>
			expect(context.programming.getSnapshot().showId).toBe(OTHER_SHOW_ID),
		);
		answer.resolve(
			groupOutcome(requestOf(context.applySelection), selection(2, ["leaked"])),
		);
		await act(async () => {
			await Promise.resolve();
			await Promise.resolve();
		});

		expect(context.programming.getSnapshot().selection).toEqual(
			selection(5, []),
		);
	});

	it("ignores selection published for another desk", async () => {
		const context = harness();
		render(view(context, <GroupStrip />));
		installGroups(context, [group("1", "Front Truss")]);
		await settleAuthority(context);
		await waitFor(() => expect(context.transport.subscriptions).toHaveLength(1));
		const before = context.programming.getSnapshot().selection;

		act(() =>
			context.transport.emit({
				type: "event",
				sequence: 20,
				correlationId: null,
				change: selectionChange({
					deskId: OTHER_DESK_ID,
					selected: ["foreign"],
				}),
			}),
		);

		expect(context.programming.getSnapshot().selection).toEqual(before);
		expect(
			screen.getByText("Front Truss").closest("button")!.className,
		).not.toContain("selected");
	});

	it("marks the activated Group selected from authoritative expression alone", async () => {
		const context = harness();
		context.applySelection.mockImplementation(
			async (_desk, request) =>
				groupOutcome(request, {
					selected: ORDERED_MEMBERS,
					expression: { type: "live_group", groupId: "1", rule: { type: "all" } },
					revision: 2,
					gestureOpen: false,
				}),
		);
		render(view(context, <GroupStrip />));
		installGroups(context, [group("1", "Front Truss")]);
		await settleAuthority(context);

		fireEvent.click(screen.getByText("Front Truss").closest("button")!);

		await waitFor(() =>
			expect(
				screen.getByText("Front Truss").closest("button")!.className,
			).toContain("selected"),
		);
		expect(screen.getByText("3 fixtures")).toBeInTheDocument();
	});

	it("renders the Group Strip without reading runtime Playback master feedback", async () => {
		const context = harness();
		render(view(context, <GroupStrip />));
		installGroups(context, [group("1", "Front Truss")]);
		await settleAuthority(context);

		expect(screen.getByText("Front Truss")).toBeInTheDocument();
		expect(screen.getByText("3 fixtures")).toBeInTheDocument();
		expect(mocks.playbackReads).toBe(0);
	});

	it("opens no Programming authority for an inactive Group Strip", async () => {
		const context = harness();
		render(view(context, <GroupStrip active={false} />));
		installGroups(context, [group("1", "Front Truss")]);
		await act(async () => {
			await Promise.resolve();
		});

		expect(context.loadSnapshot).not.toHaveBeenCalled();
		expect(context.transport.subscriptions).toHaveLength(0);
	});

	it("opens no Programming authority for an inactive Group Pool", async () => {
		const context = harness();
		render(view(context, <GroupPool active={false} cards={[group("1", "Front Truss")]} />));
		installGroups(context, [group("1", "Front Truss")]);
		await act(async () => {
			await Promise.resolve();
		});

		expect(context.loadSnapshot).not.toHaveBeenCalled();
		expect(context.transport.subscriptions).toHaveLength(0);
	});

	it("activates a Group Pool card through the same scoped gesture", async () => {
		const context = harness();
		const answer = deferred<SelectionActionOutcome>();
		context.applySelection.mockReturnValue(answer.promise);
		const pool = group("1", "Front Truss");
		render(view(context, <GroupPool cards={[pool]} />));
		installGroups(context, [pool]);
		await settleAuthority(context);

		fireEvent.click(screen.getByText("Front Truss").closest("button")!);

		await waitFor(() => expect(context.applySelection).toHaveBeenCalledOnce());
		expect(requestOf(context.applySelection).action).toEqual({
			type: "gesture",
			source: { type: "live_group", groupId: "1" },
			remove: false,
		});
		expect(context.programming.getSnapshot().selection?.selected).toEqual(
			ORDERED_MEMBERS,
		);
	});

	it("sends no Group Pool selection while Group authority is missing", async () => {
		const context = harness();
		render(view(context, <GroupPool cards={[group("1", "Front Truss")]} />));
		await settleAuthority(context);

		fireEvent.click(screen.getByText("Front Truss").closest("button")!);

		expect(context.applySelection).not.toHaveBeenCalled();
	});
});
