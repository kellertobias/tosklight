import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	ProgrammingUpdateActionOutcome,
	ProgrammingUpdateActionRequest,
	ProgrammingUpdateTransport,
} from "./contracts";
import {
	ProgrammingUpdateProvider,
	useProgrammingUpdate,
} from "./ProgrammingUpdateProvider";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const USER_ID = "33333333-3333-4333-8333-333333333333";
let consumerRenders = 0;

afterEach(cleanup);

function Consumer() {
	consumerRenders += 1;
	const update = useProgrammingUpdate();
	return (
		<button
			type="button"
			onClick={() =>
				void update?.applyDirect(
					{ family: { type: "group" }, object_id: "front" },
					{ target_type: "existing_content", mode: "update_existing" },
				)
			}
		>
			Update
		</button>
	);
}

function transport(apply = vi.fn()): ProgrammingUpdateTransport {
	return {
		preview: vi.fn(),
		targets: vi.fn(),
		apply,
		loadSettings: vi.fn(),
		saveSettings: vi.fn(),
	};
}

function changed(
	request: ProgrammingUpdateActionRequest,
): ProgrammingUpdateActionOutcome {
	return {
		status: "changed",
		request_id: request.request_id,
		correlation_id: "44444444-4444-4444-8444-444444444444",
		replayed: false,
		show_id: SHOW_ID,
		show_revision: 8,
		projection: {
			kind: "group",
			object_id: "front",
			object_revision: 2,
			body: { name: "Updated", fixtures: [] },
		},
		event_sequence: 12,
		summary: {
			target: {
				family: { type: "group" },
				object_id: "front",
				name: "Front",
			},
			revision_before: 1,
			revision_after: 2,
			eligible_count: 1,
			changed_count: 1,
			added_count: 0,
			ignored_count: 0,
			changed_cues: [],
			programmer_values_retained: false,
		},
	};
}

function renderProvider(
	store: ShowObjectsStore,
	actionTransport: ProgrammingUpdateTransport,
	loadObject = vi.fn(async () => null),
	children = <Consumer />,
	strict = false,
) {
	const provider = (
		<ProgrammingUpdateProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			userId={USER_ID}
			initialShowRevision={7}
			authorityKey="server-a|session-a"
			store={store}
			transport={actionTransport}
			loadObject={loadObject}
		>
			{children}
		</ProgrammingUpdateProvider>
	);
	return render(strict ? <StrictMode>{provider}</StrictMode> : provider);
}

describe("ProgrammingUpdateProvider", () => {
	it("is dormant on mount and does not rerender consumers for unrelated store changes", async () => {
		consumerRenders = 0;
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "group", [], 10, 7);
		const actionTransport = transport();
		const loadObject = vi.fn(async () => null);

		renderProvider(store, actionTransport, loadObject);
		await Promise.resolve();

		expect(actionTransport.preview).not.toHaveBeenCalled();
		expect(actionTransport.targets).not.toHaveBeenCalled();
		expect(actionTransport.apply).not.toHaveBeenCalled();
		expect(actionTransport.loadSettings).not.toHaveBeenCalled();
		expect(actionTransport.saveSettings).not.toHaveBeenCalled();
		expect(loadObject).not.toHaveBeenCalled();
		expect(consumerRenders).toBe(1);

		act(() => store.setCollection(SHOW_ID, "preset", [], 11, 8));
		expect(consumerRenders).toBe(1);
	});

	it("keeps the memoized writer live through StrictMode effect replay", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "group", [], 10, 7);
		const apply = vi.fn(
			async (
				_show: string,
				_revision: number,
				request: ProgrammingUpdateActionRequest,
			) => changed(request),
		);
		const actionTransport = transport(apply);

		renderProvider(
			store,
			actionTransport,
			vi.fn(async () => null),
			<Consumer />,
			true,
		);
		await Promise.resolve();
		fireEvent.click(screen.getByRole("button", { name: "Update" }));

		await waitFor(() => expect(apply).toHaveBeenCalledOnce());
		expect(store.getSnapshot().groups[0]).toMatchObject({
			id: "front",
			revision: 2,
			body: { name: "Updated" },
		});
	});
});
