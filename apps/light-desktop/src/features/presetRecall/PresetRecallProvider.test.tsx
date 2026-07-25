import { act, render, waitFor } from "@testing-library/react";
import { memo } from "react";
import { describe, expect, it, vi } from "vitest";
import { ProgrammerCaptureModeViewProvider } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { FakeProgrammerCaptureModeTransport } from "../programmerCaptureMode/testFixtures";
import { ProgrammerValuesViewProvider } from "../programmerValues/ProgrammerValuesView";
import { ProgrammerValuesStore } from "../programmerValues/store";
import { FakeProgrammerValuesTransport } from "../programmerValues/testFixtures";
import { ProgrammingInteractionViewProvider } from "../programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../programmingInteraction/store";
import { FakeProgrammingTransport } from "../programmingInteraction/testFixtures";
import type { ShowObjectKind } from "../showObjects/contracts";
import { ShowObjectsViewProvider } from "../showObjects/ShowObjectsView";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventTransport,
} from "../showObjects/transport";
import { PresetRecallProvider, usePresetRecall } from "./PresetRecallProvider";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const FIXTURE_ID = "44444444-4444-4444-8444-444444444444";

class FakeShowTransport implements ShowObjectsEventTransport {
	readonly subscriptions: Array<{
		scope: ShowObjectsEventScope;
		close: ReturnType<typeof vi.fn>;
	}> = [];

	subscribe(
		_showId: string,
		scope: ShowObjectsEventScope,
		_afterSequence: number | null,
		_observer: ShowObjectsEventObserver,
	) {
		const close = vi.fn();
		this.subscriptions.push({ scope, close });
		return { close, repair: vi.fn() };
	}
}

let unrelatedRenders = 0;

const UnrelatedConsumer = memo(function UnrelatedConsumer() {
	unrelatedRenders += 1;
	return null;
});

function PresetRecallProbe({ enabled }: { enabled: boolean }) {
	const recall = usePresetRecall(enabled);
	return <span>{recall.selection?.revision ?? "Dormant"}</span>;
}

describe("PresetRecallProvider", () => {
	it("keeps every exact authority dormant until the first enabled Presets view", async () => {
		unrelatedRenders = 0;
		const showStore = new ShowObjectsStore();
		const valuesStore = new ProgrammerValuesStore();
		const captureModeStore = new ProgrammerCaptureModeStore();
		const programmingStore = new ProgrammingInteractionStore();
		const showTransport = new FakeShowTransport();
		const valuesTransport = new FakeProgrammerValuesTransport();
		const captureModeTransport = new FakeProgrammerCaptureModeTransport();
		const programmingTransport = new FakeProgrammingTransport();
		const loadCollection = vi.fn(
			async (_showId: string, kind: ShowObjectKind) => ({
				objects:
					kind === "preset"
						? [
								{
									kind: "preset" as const,
									id: "2.7",
									revision: 4,
									updated_at: "",
									body: {
										name: "Blue",
										number: 7,
										family: "Color" as const,
										values: {},
									},
								},
							]
						: [],
				showRevision: 12,
			}),
		);
		const loadValues = vi.fn(async () => ({
			cursor: 30,
			projection: {
				userId: USER_ID,
				revision: 6,
				fixtureValues: [],
				groupValues: [],
			},
		}));
		const loadCaptureMode = vi.fn(async () => ({
			cursor: 30,
			projection: {
				userId: USER_ID,
				revision: 3,
				blind: false,
				preview: false,
				preloadCaptureProgrammer: false,
			},
		}));
		const loadProgramming = vi.fn(async () => ({
			cursor: 30,
			projection: {
				deskId: DESK_ID,
				commandLine: {
					text: "FIXTURE",
					target: "FIXTURE" as const,
					pristine: true,
					revision: 1,
					pendingChoice: null,
				},
				selection: {
					selected: [FIXTURE_ID],
					expression: { type: "static" as const },
					revision: 8,
					gestureOpen: false,
				},
			},
		}));
		const recall = vi.fn();
		const loadPreset = vi.fn();
		const loadObject = vi.fn();
		const presetTransport = { recall };
		const view = (enabled: boolean) => (
			<ShowObjectsViewProvider
				showId={SHOW_ID}
				authorityKey="session-a"
				store={showStore}
				transport={showTransport}
				loadCollection={loadCollection}
				loadObject={loadObject}
			>
				<ProgrammerCaptureModeViewProvider
					showId={SHOW_ID}
					userId={USER_ID}
					authorityKey="session-a"
					store={captureModeStore}
					transport={captureModeTransport}
					loadSnapshot={loadCaptureMode}
				>
					<ProgrammerValuesViewProvider
						showId={SHOW_ID}
						userId={USER_ID}
						authorityKey="session-a"
						store={valuesStore}
						transport={valuesTransport}
						loadSnapshot={loadValues}
					>
						<ProgrammingInteractionViewProvider
							showId={SHOW_ID}
							deskId={DESK_ID}
							authorityKey="session-a"
							store={programmingStore}
							transport={programmingTransport}
							loadSnapshot={loadProgramming}
						>
							<PresetRecallProvider
								showId={SHOW_ID}
								userId={USER_ID}
								deskId={DESK_ID}
								authorityKey="session-a"
								showStore={showStore}
								transport={presetTransport}
								loadPreset={loadPreset}
							>
								<PresetRecallProbe enabled={enabled} />
								<UnrelatedConsumer />
							</PresetRecallProvider>
						</ProgrammingInteractionViewProvider>
					</ProgrammerValuesViewProvider>
				</ProgrammerCaptureModeViewProvider>
			</ShowObjectsViewProvider>
		);
		const rendered = render(view(false));

		await act(async () => Promise.resolve());
		expect(loadCollection).not.toHaveBeenCalled();
		expect(loadValues).not.toHaveBeenCalled();
		expect(loadCaptureMode).not.toHaveBeenCalled();
		expect(loadProgramming).not.toHaveBeenCalled();
		expect(showTransport.subscriptions).toHaveLength(0);
		expect(valuesTransport.subscriptions).toHaveLength(0);
		expect(captureModeTransport.subscriptions).toHaveLength(0);
		expect(programmingTransport.subscriptions).toHaveLength(0);
		expect(recall).not.toHaveBeenCalled();
		expect(loadPreset).not.toHaveBeenCalled();

		rendered.rerender(view(true));
		await waitFor(() => expect(loadCollection).toHaveBeenCalledOnce());
		await waitFor(() => expect(loadValues).toHaveBeenCalledOnce());
		await waitFor(() => expect(loadCaptureMode).toHaveBeenCalledOnce());
		await waitFor(() => expect(loadProgramming).toHaveBeenCalledOnce());
		await waitFor(() => expect(showTransport.subscriptions).toHaveLength(1));
		await waitFor(() => expect(valuesTransport.subscriptions).toHaveLength(1));
		await waitFor(() =>
			expect(captureModeTransport.subscriptions).toHaveLength(1),
		);
		await waitFor(() =>
			expect(programmingTransport.subscriptions).toHaveLength(1),
		);
		expect(showTransport.subscriptions[0].scope).toEqual({
			kinds: ["preset"],
			objects: [],
		});
		expect(programmingTransport.subscriptions[0].scope).toEqual({
			commandLine: false,
			selection: true,
		});
		expect(recall).not.toHaveBeenCalled();
		expect(loadPreset).not.toHaveBeenCalled();
		expect(unrelatedRenders).toBe(1);
	});
});
