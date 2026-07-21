import { describe, expect, it, vi } from "vitest";
import type {
	CommandLineProjection,
	PendingCommandChoice,
} from "../programmingInteraction/contracts";
import { ProgrammingInteractionStore } from "../programmingInteraction/store";
import type { ShowObject } from "../showObjects/contracts";
import { ShowObjectsStore } from "../showObjects/store";
import {
	type CueTransferActionOutcome,
	type CueTransferActionRequest,
	type CueTransferConflictRepair,
	type CueTransferTransport,
	CueTransferTransportError,
} from "./contracts";
import { CueTransferWriter } from "./writer";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const USER_ID = "33333333-3333-4333-8333-333333333333";
const CHOICE_ID = "44444444-4444-4444-8444-444444444444";
const SOURCE_LIST_ID = "55555555-5555-4555-8555-555555555555";
const DESTINATION_LIST_ID = "66666666-6666-4666-8666-666666666666";
const SOURCE_CUE_ID = "77777777-7777-4777-8777-777777777777";
const DESTINATION_CUE_ID = "88888888-8888-4888-8888-888888888888";

const choice: PendingCommandChoice = {
	type: "cue_move_copy",
	choiceId: CHOICE_ID,
	showId: SHOW_ID,
	showRevision: 7,
	operation: "copy",
	command: "COPY SET 1 CUE 1 AT SET 2 CUE 2",
	options: [
		{ id: "plain", label: "Plain Copy", command: "COPY PLAIN" },
		{ id: "status", label: "Status Copy", command: "COPY STATUS" },
	],
	cancelLabel: "Cancel",
};

function commandLine(
	revision: number,
	pendingChoice: PendingCommandChoice | null,
): CommandLineProjection {
	return {
		text: pendingChoice?.command ?? "FIXTURE",
		target: "FIXTURE",
		pristine: pendingChoice == null,
		revision,
		pendingChoice,
	};
}

function cueList(
	objectId: string,
	cueListId: string,
	revision: number,
	name: string,
	cues: Array<{ id: string; number: number }>,
): ShowObject<"cue_list"> {
	return {
		kind: "cue_list",
		id: objectId,
		revision,
		updated_at: "",
		body: {
			id: cueListId,
			name,
			priority: 0,
			mode: "sequence",
			looped: false,
			cues: cues.map((cue) => ({
				...cue,
				name: `Cue ${cue.number}`,
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" as const },
				cue_only: false,
				changes: [],
				group_changes: [],
				phasers: [],
			})),
		},
	};
}

function outcome(
	request: CueTransferActionRequest,
	name = "Response",
): CueTransferActionOutcome {
	const destination = cueList("destination", DESTINATION_LIST_ID, 2, name, [
		{ id: DESTINATION_CUE_ID, number: 2 },
	]);
	return {
		requestId: request.requestId,
		choiceId: request.choiceId,
		correlationId: "99999999-9999-4999-8999-999999999999",
		replayed: false,
		showId: SHOW_ID,
		summary: {
			operation: "copy",
			mode: request.mode,
			sourceCueId: SOURCE_CUE_ID,
			sourceCueNumber: 1,
			destinationCueId: DESTINATION_CUE_ID,
			destinationCueNumber: 2,
		},
		showRevision: 8,
		projections: [
			{
				cueListId: DESTINATION_LIST_ID,
				objectId: destination.id,
				objectRevision: destination.revision,
				body: destination.body,
			},
		],
		showEventSequence: 12,
		commandLine: commandLine(2, null),
		interactionEventSequence: 21,
		persistenceWarning: null,
	};
}

function setup(
	apply: CueTransferTransport["apply"],
	repairOverrides: Partial<CueTransferConflictRepair> = {},
) {
	const showStore = new ShowObjectsStore();
	showStore.reset(SHOW_ID, "session-a");
	showStore.setCollection(
		SHOW_ID,
		"cue_list",
		[
			cueList("source", SOURCE_LIST_ID, 1, "Source", [
				{ id: SOURCE_CUE_ID, number: 1 },
			]),
			cueList("destination", DESTINATION_LIST_ID, 1, "Destination", []),
		],
		10,
		7,
	);
	const programmingStore = new ProgrammingInteractionStore();
	programmingStore.reset(SHOW_ID, DESK_ID, "session-a");
	programmingStore.installSnapshot({
		cursor: 20,
		projection: {
			deskId: DESK_ID,
			commandLine: commandLine(1, choice),
			selection: {
				selected: [],
				expression: null,
				revision: 1,
				gestureOpen: false,
			},
		},
	});
	const onError = vi.fn();
	const repair: CueTransferConflictRepair = {
		loadCueLists: vi.fn(async () => {
			const snapshot = showStore.getSnapshot();
			return {
				objects: [...snapshot.cueLists],
				showRevision: snapshot.showRevision ?? 0,
			};
		}),
		loadCommandLine: vi.fn(async () => {
			const current = programmingStore.getSnapshot().commandLine;
			if (!current) throw new Error("Command line is unavailable");
			return current;
		}),
		...repairOverrides,
	};
	const writer = new CueTransferWriter({
		scope: { showId: SHOW_ID, deskId: DESK_ID, userId: USER_ID },
		showStore,
		programmingStore,
		transport: { apply },
		repair,
		onError,
	});
	return { showStore, programmingStore, repair, writer, onError };
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

describe("CueTransferWriter", () => {
	it("closes optimistically and installs one response before its retained events", async () => {
		const apply = vi.fn(async (_show, _revision, request) => outcome(request));
		const { showStore, programmingStore, writer } = setup(apply);
		const unrelatedGroups = showStore.getSnapshot().groups;

		await expect(writer.apply(choice, "plain")).resolves.toBe(true);
		expect(apply).toHaveBeenCalledWith(
			SHOW_ID,
			7,
			expect.objectContaining({
				choiceId: CHOICE_ID,
				mode: "plain",
				expectedCommandLineRevision: 1,
			}),
		);
		expect(programmingStore.getSnapshot().commandLine).toEqual(
			commandLine(2, null),
		);
		expect(destinationName(showStore)).toBe("Response");
		expect(showStore.getSnapshot().groups).toBe(unrelatedGroups);

		showStore.applyChange(showChange("Duplicate event"));
		expect(destinationName(showStore)).toBe("Response");
	});

	it("retains authoritative events that arrive before the HTTP response", async () => {
		const pending = deferred<CueTransferActionOutcome>();
		const apply = vi.fn<CueTransferTransport["apply"]>(
			async () => pending.promise,
		);
		const { showStore, programmingStore, writer } = setup(apply);
		const writing = writer.apply(choice, "status");
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const request = apply.mock.calls[0][2];

		showStore.applyChange(showChange("Event first"));
		programmingStore.applyChange(
			{ deskId: DESK_ID, commandLine: commandLine(2, null) },
			21,
		);
		const eventObjects = showStore.getSnapshot().cueLists;
		pending.resolve(outcome(request, "Late response"));

		await expect(writing).resolves.toBe(true);
		expect(showStore.getSnapshot().cueLists).toBe(eventObjects);
		expect(destinationName(showStore)).toBe("Event first");
	});

	it("retries once with the identical action and accepts its replay", async () => {
		const unavailable = new CueTransferTransportError(
			"unavailable",
			503,
			null,
			null,
			true,
		);
		const apply = vi
			.fn()
			.mockRejectedValueOnce(unavailable)
			.mockImplementation(async (_show, _revision, request) => ({
				...outcome(request),
				replayed: true,
			}));
		const { writer } = setup(apply);

		await expect(writer.apply(choice, "plain")).resolves.toBe(true);
		expect(apply).toHaveBeenCalledTimes(2);
		expect(apply.mock.calls[1][1]).toBe(apply.mock.calls[0][1]);
		expect(apply.mock.calls[1][2]).toBe(apply.mock.calls[0][2]);
	});

	it("rolls back immediately and narrowly repairs Cuelists after a Show conflict", async () => {
		const failure = new CueTransferTransportError(
			"conflict",
			409,
			9,
			null,
			false,
		);
		const action = deferred<CueTransferActionOutcome>();
		const repaired = cueList(
			"destination",
			DESTINATION_LIST_ID,
			3,
			"Conflict repair",
			[],
		);
		const loadCueLists = vi.fn(async () => ({
			objects: [repaired],
			showRevision: 9,
		}));
		const { showStore, programmingStore, repair, writer, onError } = setup(
			vi.fn(async () => action.promise),
			{ loadCueLists },
		);
		const writing = writer.apply(choice, "plain");
		await vi.waitFor(() =>
			expect(
				programmingStore.getSnapshot().commandLine?.pendingChoice,
			).toBeNull(),
		);
		action.reject(failure);

		await expect(writing).resolves.toBe(false);
		expect(programmingStore.getSnapshot().commandLine?.pendingChoice).toEqual(
			choice,
		);
		expect(showStore.getSnapshot().showRevision).toBe(9);
		expect(destinationName(showStore)).toBe("Conflict repair");
		expect(loadCueLists).toHaveBeenCalledWith(SHOW_ID);
		expect(repair.loadCommandLine).not.toHaveBeenCalled();
		expect(onError).toHaveBeenCalledWith(failure);
	});

	it("repairs only the current command-line choice after its revision conflicts", async () => {
		const failure = new CueTransferTransportError(
			"command line changed",
			409,
			null,
			3,
			false,
		);
		const currentChoice = {
			...choice,
			choiceId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
		};
		const loadCommandLine = vi.fn(async () => commandLine(3, currentChoice));
		const { programmingStore, repair, writer } = setup(
			vi.fn(async () => {
				throw failure;
			}),
			{ loadCommandLine },
		);

		await expect(writer.apply(choice, "status")).resolves.toBe(false);
		expect(programmingStore.getSnapshot().commandLine).toEqual(
			commandLine(3, currentChoice),
		);
		expect(loadCommandLine).toHaveBeenCalledWith(DESK_ID);
		expect(repair.loadCueLists).not.toHaveBeenCalled();
	});

	it("drops a late response after same-show authority replacement", async () => {
		const pending = deferred<CueTransferActionOutcome>();
		const apply = vi.fn<CueTransferTransport["apply"]>(
			async () => pending.promise,
		);
		const { showStore, programmingStore, writer, onError } = setup(apply);
		const writing = writer.apply(choice, "plain");
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const request = apply.mock.calls[0][2];

		showStore.reset(SHOW_ID, "session-b");
		showStore.setCollection(SHOW_ID, "cue_list", [], 30, 20);
		programmingStore.reset(SHOW_ID, DESK_ID, "session-b");
		pending.resolve(outcome(request, "Late"));

		await expect(writing).resolves.toBe(false);
		expect(showStore.getSnapshot().cueLists).toEqual([]);
		expect(onError).not.toHaveBeenCalled();
	});

	it("allows only one network action for a pending choice", async () => {
		const pending = deferred<CueTransferActionOutcome>();
		const apply = vi.fn<CueTransferTransport["apply"]>(
			async () => pending.promise,
		);
		const { writer } = setup(apply);
		const first = writer.apply(choice, "plain");
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());

		await expect(writer.apply(choice, "status")).resolves.toBe(false);
		const request = apply.mock.calls[0][2];
		pending.resolve(outcome(request));
		await expect(first).resolves.toBe(true);
		expect(apply).toHaveBeenCalledOnce();
	});
});

function showChange(name: string) {
	const destination = cueList("destination", DESTINATION_LIST_ID, 2, name, [
		{ id: DESTINATION_CUE_ID, number: 2 },
	]);
	return {
		showId: SHOW_ID,
		showRevision: 8,
		eventSequence: 12,
		changes: [
			{
				kind: "cue_list" as const,
				objectId: destination.id,
				objectRevision: destination.revision,
				body: destination.body,
				deleted: false,
			},
		],
	};
}

function destinationName(store: ShowObjectsStore) {
	return store.getSnapshot().cueLists.find(({ id }) => id === "destination")
		?.body.name;
}
