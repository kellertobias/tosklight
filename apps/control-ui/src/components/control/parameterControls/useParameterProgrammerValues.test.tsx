import { act, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import type { ProgrammerValuesSnapshot } from "../../../features/programmerValues/contracts";
import { ProgrammerValuesViewProvider } from "../../../features/programmerValues/ProgrammerValuesView";
import { ProgrammerValuesStore } from "../../../features/programmerValues/store";
import {
	FakeProgrammerValuesTransport,
	FIXTURE_1,
	FIXTURE_2,
	fixtureValue,
	groupValue,
	SHOW_ID,
	USER_ID,
	valuesProjection,
} from "../../../features/programmerValues/testFixtures";
import { useParameterProgrammerValues } from "./useParameterProgrammerValues";

const FIXTURE_3 = "33333333-3333-4333-8333-333333333333";
const DEFAULT_FIXTURE_IDS = [FIXTURE_1, FIXTURE_2] as const;

function Probe({
	enabled,
	onRender,
	fixtureIds = DEFAULT_FIXTURE_IDS,
	groupId = "front",
	testId,
}: {
	enabled: boolean;
	onRender: () => void;
	fixtureIds?: readonly string[];
	groupId?: string | null;
	testId?: string;
}) {
	onRender();
	const view = useParameterProgrammerValues(fixtureIds, groupId, enabled);
	if (!view) return <span>Disabled</span>;
	return (
		<span data-testid={testId}>
			{view.ready ? "Ready" : "Pending"}:
			{view.fixtureValues.map(valueKey).join(",")}:
			{view.groupValues.map(valueKey).join(",")}
		</span>
	);
}

function valueKey(value: {
	attribute: string;
	value: { kind: string; value: unknown };
}) {
	return `${value.attribute}=${String(value.value.value)}`;
}

function deferredSnapshot() {
	let resolve!: (snapshot: ProgrammerValuesSnapshot) => void;
	const promise = new Promise<ProgrammerValuesSnapshot>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

function provider(
	children: ReactNode,
	store: ProgrammerValuesStore,
	transport: FakeProgrammerValuesTransport,
	loadSnapshot: () => Promise<ProgrammerValuesSnapshot>,
) {
	return (
		<ProgrammerValuesViewProvider
			showId={SHOW_ID}
			userId={USER_ID}
			store={store}
			transport={transport}
			loadSnapshot={loadSnapshot}
		>
			{children}
		</ProgrammerValuesViewProvider>
	);
}

describe("useParameterProgrammerValues", () => {
	it("is dormant while disabled and selects only authoritative parameter values", async () => {
		const store = new ProgrammerValuesStore();
		const transport = new FakeProgrammerValuesTransport();
		const snapshot = deferredSnapshot();
		const loadSnapshot = vi.fn(() => snapshot.promise);
		const onRender = vi.fn();
		const view = (enabled: boolean) =>
			provider(
				<Probe enabled={enabled} onRender={onRender} />,
				store,
				transport,
				loadSnapshot,
			);
		const rendered = render(view(false));

		expect(screen.getByText("Disabled")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(view(true));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledOnce());
		expect(screen.getByText("Pending::")).toBeInTheDocument();

		await act(async () =>
			snapshot.resolve({
				cursor: 10,
				projection: valuesProjection({
					fixtureValues: [
						fixtureValue(0.25),
						fixtureValue(0.5, {
							fixtureId: FIXTURE_2,
							attribute: "pan",
							programmerOrder: 2,
						}),
						fixtureValue(0.9, {
							fixtureId: FIXTURE_3,
							attribute: "zoom",
							programmerOrder: 3,
						}),
					],
					groupValues: [
						groupValue(0.6),
						groupValue(0.8, { groupId: "back", programmerOrder: 4 }),
					],
				}),
			}),
		);

		await waitFor(() =>
			expect(
				screen.getByText("Ready:intensity=0.25,pan=0.5:intensity=0.6"),
			).toBeInTheDocument(),
		);
		expect(transport.subscriptions).toHaveLength(1);

		const readyRenderCount = onRender.mock.calls.length;
		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: "unrelated-values",
				projection: valuesProjection({
					revision: 2,
					fixtureValues: [
						fixtureValue(0.25),
						fixtureValue(0.5, {
							fixtureId: FIXTURE_2,
							attribute: "pan",
							programmerOrder: 2,
						}),
						fixtureValue(0.1, {
							fixtureId: FIXTURE_3,
							attribute: "zoom",
							programmerOrder: 3,
						}),
					],
					groupValues: [
						groupValue(0.6),
						groupValue(0.2, { groupId: "back", programmerOrder: 4 }),
					],
				}),
			}),
		);

		expect(onRender).toHaveBeenCalledTimes(readyRenderCount);
	});

	it("reselects fixture and Group values without waiting for a values-store change", async () => {
		const store = new ProgrammerValuesStore();
		const transport = new FakeProgrammerValuesTransport();
		const loadSnapshot = vi.fn(async () => ({
			cursor: 10,
			projection: valuesProjection({
				fixtureValues: [
					fixtureValue(0.25),
					fixtureValue(0.5, {
						fixtureId: FIXTURE_2,
						attribute: "pan",
						programmerOrder: 2,
					}),
				],
				groupValues: [
					groupValue(0.6),
					groupValue(0.8, { groupId: "back", programmerOrder: 4 }),
				],
			}),
		}));
		const view = (fixtureIds: readonly string[], groupId: string) =>
			provider(
				<Probe
					enabled
					onRender={vi.fn()}
					fixtureIds={fixtureIds}
					groupId={groupId}
				/>,
				store,
				transport,
				loadSnapshot,
			);
		const rendered = render(view([FIXTURE_1], "front"));

		await waitFor(() =>
			expect(
				screen.getByText("Ready:intensity=0.25:intensity=0.6"),
			).toBeInTheDocument(),
		);
		rendered.rerender(view([FIXTURE_2], "back"));

		expect(screen.getByText("Ready:pan=0.5:intensity=0.8")).toBeInTheDocument();
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions).toHaveLength(1);
	});

	it("enables another consumer immediately after the shared session is hydrated", async () => {
		const store = new ProgrammerValuesStore();
		const transport = new FakeProgrammerValuesTransport();
		const loadSnapshot = vi.fn(async () => ({
			cursor: 10,
			projection: valuesProjection({
				fixtureValues: [fixtureValue(0.25)],
				groupValues: [groupValue(0.6)],
			}),
		}));
		const view = (secondEnabled: boolean) =>
			provider(
				<>
					<Probe enabled onRender={vi.fn()} testId="first-values" />
					<Probe
						enabled={secondEnabled}
						onRender={vi.fn()}
						testId="second-values"
					/>
				</>,
				store,
				transport,
				loadSnapshot,
			);
		const rendered = render(view(false));

		await waitFor(() =>
			expect(screen.getByTestId("first-values")).toHaveTextContent(
				"Ready:intensity=0.25:intensity=0.6",
			),
		);
		rendered.rerender(view(true));

		expect(screen.getByTestId("second-values")).toHaveTextContent(
			"Ready:intensity=0.25:intensity=0.6",
		);
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions).toHaveLength(1);
	});
});
