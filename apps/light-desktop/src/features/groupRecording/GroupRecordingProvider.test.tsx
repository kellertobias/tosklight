import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ShowObjectsStore } from "../showObjects/store";
import type { GroupRecordingRequest } from "./contracts";
import {
	GroupRecordingProvider,
	useGroupRecording,
} from "./GroupRecordingProvider";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
let unrelatedRenders = 0;

function UnrelatedConsumer() {
	unrelatedRenders += 1;
	return null;
}

function RecordButton() {
	const actions = useGroupRecording();
	return (
		<button
			type="button"
			onClick={() =>
				void actions?.record({
					objectId: "front",
					operation: "overwrite",
					expectedObjectRevision: 0,
				})
			}
		>
			Record
		</button>
	);
}

describe("GroupRecordingProvider", () => {
	it("does no I/O or unrelated rerender until an action is invoked", async () => {
		unrelatedRenders = 0;
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "group", []);
		const record = vi.fn(
			async (_showId: string, request: GroupRecordingRequest) => ({
				requestId: request.requestId,
				correlationId: "33333333-3333-4333-8333-333333333333",
				replayed: false,
				status: "changed" as const,
				showRevision: 2,
				group: {
					state: "stored" as const,
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
				eventSequence: 4,
			}),
		);
		const loadGroup = vi.fn(async () => null);
		render(
			<GroupRecordingProvider
				showId={SHOW_ID}
				store={store}
				transport={{ record }}
				loadGroup={loadGroup}
			>
				<RecordButton />
				<UnrelatedConsumer />
			</GroupRecordingProvider>,
		);

		expect(record).not.toHaveBeenCalled();
		expect(loadGroup).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Record" }));
		await waitFor(() => expect(record).toHaveBeenCalledOnce());
		expect(loadGroup).not.toHaveBeenCalled();
		expect(unrelatedRenders).toBe(1);
	});
});
