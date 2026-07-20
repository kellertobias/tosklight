import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ShowObjectsStore } from "../showObjects/store";
import type { PresetRecordingRequest } from "./contracts";
import {
	PresetRecordingProvider,
	usePresetRecording,
} from "./PresetRecordingProvider";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
let unrelatedRenders = 0;

function UnrelatedConsumer() {
	unrelatedRenders += 1;
	return null;
}

function RecordButton() {
	const actions = usePresetRecording();
	return (
		<button
			type="button"
			onClick={() =>
				void actions?.record({
					objectId: "2.1",
					address: { family: "Color", number: 1 },
					name: "Blue",
					mode: "overwrite",
					expectedObjectRevision: 0,
				})
			}
		>
			Record
		</button>
	);
}

describe("PresetRecordingProvider", () => {
	it("does no I/O until an action is invoked", async () => {
		unrelatedRenders = 0;
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "preset", []);
		const record = vi.fn(async (_showId: string, request: PresetRecordingRequest) => ({
			requestId: request.requestId,
			correlationId: "33333333-3333-4333-8333-333333333333",
			replayed: false,
			status: "changed" as const,
			showRevision: 2,
			preset: {
				kind: "preset" as const,
				id: "2.1",
				revision: 1,
				updated_at: "",
				body: { name: "Blue", number: 1, family: "Color" as const, values: {} },
			},
			eventSequence: 4,
		}));
		const loadPreset = vi.fn(async () => null);
		render(
			<PresetRecordingProvider
				showId={SHOW_ID}
				store={store}
				transport={{ record }}
				loadPreset={loadPreset}
			>
				<RecordButton />
				<UnrelatedConsumer />
			</PresetRecordingProvider>,
		);

		expect(record).not.toHaveBeenCalled();
		expect(loadPreset).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Record" }));
		await waitFor(() => expect(record).toHaveBeenCalledOnce());
		expect(loadPreset).not.toHaveBeenCalled();
		expect(unrelatedRenders).toBe(1);
	});
});
