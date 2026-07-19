import { act, render, screen, waitFor } from "@testing-library/react";
import { type ReactNode, useCallback } from "react";
import { describe, expect, it, vi } from "vitest";
import type { ProgrammerCaptureModeSnapshot } from "./contracts";
import {
	ProgrammerCaptureModeViewProvider,
	useProgrammerCaptureModeSelector,
	useProgrammerCaptureModeStatus,
	useProgrammerCaptureModeView,
} from "./ProgrammerCaptureModeView";
import {
	type ProgrammerCaptureModeState,
	ProgrammerCaptureModeStore,
} from "./store";
import {
	captureModeProjection,
	captureModeSnapshot,
	FakeProgrammerCaptureModeTransport,
	OTHER_SHOW_ID,
	OTHER_USER_ID,
	SHOW_ID,
	USER_ID,
} from "./testFixtures";

function ProjectionProbe({ enabled }: { enabled: boolean }) {
	const projection = useProgrammerCaptureModeView(enabled);
	return (
		<span>{enabled ? (projection?.revision ?? "Loading") : "Hidden"}</span>
	);
}

function StatusProbe() {
	const status = useProgrammerCaptureModeStatus();
	return <span>{status.status}</span>;
}

type CaptureBoolean = "blind" | "preview" | "preloadCaptureProgrammer";

function BooleanProbe({
	field,
	equal,
	onRender,
}: {
	field: CaptureBoolean;
	equal: (left: boolean | null, right: boolean | null) => boolean;
	onRender: () => void;
}) {
	onRender();
	const selector = useCallback(
		(state: ProgrammerCaptureModeState) => state.projection?.[field] ?? null,
		[field],
	);
	const value = useProgrammerCaptureModeSelector(selector, equal);
	return <span>{value === null ? "Loading boolean" : String(value)}</span>;
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

describe("ProgrammerCaptureModeViewProvider", () => {
	it("does no snapshot or socket work before an enabled capture-mode view", async () => {
		const store = new ProgrammerCaptureModeStore();
		const transport = new FakeProgrammerCaptureModeTransport();
		const loadSnapshot = vi.fn(async () => captureModeSnapshot());
		const provider = (child: ReactNode) => (
			<ProgrammerCaptureModeViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				{child}
			</ProgrammerCaptureModeViewProvider>
		);
		const rendered = render(provider(<StatusProbe />));

		expect(screen.getByText("idle")).toBeInTheDocument();
		await act(async () => Promise.resolve());
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(provider(<ProjectionProbe enabled={false} />));
		await act(async () => Promise.resolve());
		expect(loadSnapshot).not.toHaveBeenCalled();
		rendered.rerender(provider(<ProjectionProbe enabled />));
		await waitFor(() => expect(screen.getByText("1")).toBeInTheDocument());
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions).toHaveLength(1);
	});

	it("suppresses unrelated updates and keys cache by selector and equality", async () => {
		const store = new ProgrammerCaptureModeStore();
		const transport = new FakeProgrammerCaptureModeTransport();
		const onRender = vi.fn();
		const alwaysEqual = () => true;
		const loadSnapshot = vi.fn(async () =>
			captureModeSnapshot({ preview: true }),
		);
		const provider = (
			field: CaptureBoolean,
			equal: (left: boolean | null, right: boolean | null) => boolean,
		) => (
			<ProgrammerCaptureModeViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<BooleanProbe field={field} equal={equal} onRender={onRender} />
			</ProgrammerCaptureModeViewProvider>
		);
		const rendered = render(provider("preview", Object.is));
		await waitFor(() => expect(screen.getByText("true")).toBeInTheDocument());
		const renderCount = onRender.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: "preload-switch",
				projection: captureModeProjection({
					revision: 2,
					preview: true,
					preloadCaptureProgrammer: true,
				}),
			}),
		);
		expect(onRender).toHaveBeenCalledTimes(renderCount);

		rendered.rerender(provider("blind", Object.is));
		expect(screen.getByText("false")).toBeInTheDocument();
		rendered.rerender(provider("preview", Object.is));
		expect(screen.getByText("true")).toBeInTheDocument();
		rendered.rerender(provider("preview", alwaysEqual));
		expect(screen.getByText("true")).toBeInTheDocument();
		act(() =>
			transport.emit({
				type: "event",
				sequence: 12,
				correlationId: "preview-off",
				projection: captureModeProjection({ revision: 3, preview: false }),
			}),
		);
		expect(screen.getByText("true")).toBeInTheDocument();
		rendered.rerender(provider("preview", Object.is));
		expect(screen.getByText("false")).toBeInTheDocument();
	});

	it("replaces show, user, and server authority without late snapshot leakage", async () => {
		const store = new ProgrammerCaptureModeStore();
		const transport = new FakeProgrammerCaptureModeTransport();
		const first = deferred<ProgrammerCaptureModeSnapshot>();
		const second = deferred<ProgrammerCaptureModeSnapshot>();
		const third = deferred<ProgrammerCaptureModeSnapshot>();
		const fourth = deferred<ProgrammerCaptureModeSnapshot>();
		const loadSnapshot = vi
			.fn()
			.mockReturnValueOnce(first.promise)
			.mockReturnValueOnce(second.promise)
			.mockReturnValueOnce(third.promise)
			.mockReturnValueOnce(fourth.promise);
		const provider = (showId: string, userId: string, authorityKey: string) => (
			<ProgrammerCaptureModeViewProvider
				showId={showId}
				userId={userId}
				authorityKey={authorityKey}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<ProjectionProbe enabled />
			</ProgrammerCaptureModeViewProvider>
		);
		const rendered = render(provider(SHOW_ID, USER_ID, "server-a"));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledOnce());

		rendered.rerender(provider(OTHER_SHOW_ID, USER_ID, "server-a"));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledTimes(2));
		first.resolve(captureModeSnapshot({ revision: 99 }));
		await act(async () => Promise.resolve());
		expect(store.getSnapshot()).toMatchObject({
			showId: OTHER_SHOW_ID,
			userId: USER_ID,
			projection: null,
		});
		second.resolve(captureModeSnapshot({ revision: 2 }));
		await waitFor(() => expect(screen.getByText("2")).toBeInTheDocument());

		rendered.rerender(provider(OTHER_SHOW_ID, OTHER_USER_ID, "server-a"));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledTimes(3));
		third.resolve(captureModeSnapshot({ userId: OTHER_USER_ID, revision: 3 }));
		await waitFor(() => expect(screen.getByText("3")).toBeInTheDocument());

		rendered.rerender(provider(OTHER_SHOW_ID, OTHER_USER_ID, "server-b"));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledTimes(4));
		fourth.resolve(captureModeSnapshot({ userId: OTHER_USER_ID, revision: 4 }));
		await waitFor(() => expect(screen.getByText("4")).toBeInTheDocument());
		expect(transport.subscriptions.map(({ scope }) => scope)).toEqual([
			{ showId: OTHER_SHOW_ID, userId: USER_ID },
			{ showId: OTHER_SHOW_ID, userId: OTHER_USER_ID },
			{ showId: OTHER_SHOW_ID, userId: OTHER_USER_ID },
		]);
	});
});
