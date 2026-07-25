import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ShowObjectsStore } from "../showObjects/store";
import type { GroupManagementRequest } from "./contracts";
import {
	GroupManagementProvider,
	useGroupManagement,
} from "./GroupManagementProvider";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
let unrelatedRenders = 0;

function UnrelatedConsumer() {
	unrelatedRenders += 1;
	return null;
}

function ManageButton() {
	const actions = useGroupManagement();
	return (
		<button
			type="button"
			onClick={() =>
				void actions?.manage({
					objectId: "front",
					expectedObjectRevision: 0,
					operation: { type: "undo" },
				})
			}
		>
			Manage
		</button>
	);
}

function manageResponse(request: GroupManagementRequest) {
	return {
		requestId: request.requestId,
		correlationId: "33333333-3333-4333-8333-333333333333",
		replayed: false,
		status: "changed" as const,
		showId: SHOW_ID,
		showRevision: 2,
		group: {
			id: "front",
			revision: 1,
			object: {
				kind: "group" as const,
				id: "front",
				revision: 1,
				updated_at: "",
				body: { name: "Front", fixtures: [] },
			},
		},
		persistenceWarning: null,
		eventSequence: 4,
	};
}

describe("GroupManagementProvider", () => {
	afterEach(cleanup);

	it("does no I/O or unrelated rerender until an action is invoked", async () => {
		unrelatedRenders = 0;
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "group", []);
		const manage = vi.fn(
			async (_showId: string, request: GroupManagementRequest) =>
				manageResponse(request),
		);
		const loadGroup = vi.fn(async () => null);
		render(
			<GroupManagementProvider
				showId={SHOW_ID}
				store={store}
				transport={{ manage }}
				loadGroup={loadGroup}
			>
				<ManageButton />
				<UnrelatedConsumer />
			</GroupManagementProvider>,
		);

		expect(manage).not.toHaveBeenCalled();
		expect(loadGroup).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Manage" }));
		await waitFor(() => expect(manage).toHaveBeenCalledOnce());
		expect(loadGroup).not.toHaveBeenCalled();
		expect(unrelatedRenders).toBe(1);
	});

	it("reports an actionable error instead of acting without an open Show", async () => {
		const store = new ShowObjectsStore();
		const manage = vi.fn();
		const onError = vi.fn();
		render(
			<GroupManagementProvider
				showId={null}
				store={store}
				transport={{ manage }}
				loadGroup={vi.fn(async () => null)}
				onError={onError}
			>
				<ManageButton />
			</GroupManagementProvider>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Manage" }));
		await waitFor(() =>
			expect(onError).toHaveBeenCalledWith(
				expect.objectContaining({ message: "Group management is unavailable" }),
			),
		);
		expect(manage).not.toHaveBeenCalled();
	});
});
