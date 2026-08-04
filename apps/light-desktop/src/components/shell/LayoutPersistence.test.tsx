import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initialState } from "../../state/appReducer";
import { LayoutPersistence } from "./LayoutPersistence";

const mocks = vi.hoisted(() => ({
	app: { state: null as unknown as typeof initialState, dispatch: vi.fn() },
	server: {
		bootstrap: { active_show: { id: "show" } },
		session: { user: { id: "user" }, desk: { id: "control-desk-a" } },
		deskLayout: null as null | {
			revision: number;
			body: { desks: typeof initialState.desks; activeDeskId: string };
		},
		deskLayoutScope: null as string | null,
		saveDeskLayout: vi.fn(),
	},
}));

vi.mock("../../api/ServerContext", async (importOriginal) => {
	const original =
		await importOriginal<typeof import("../../api/ServerContext")>();
	return { ...original, useServer: () => mocks.server };
});
vi.mock(
	"../../features/deskSnapshot/DeskSnapshotState",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useActiveShowId: () => mocks.server.bootstrap?.active_show?.id ?? null,
		useSessionSnapshot: () => mocks.server.session ?? null,
	}),
);
vi.mock(
	"../../features/deskConnection/DeskConnectionContext",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useDeskConnection: () => mocks.server,
	}),
);

vi.mock("../../state/AppContext", () => ({
	useApp: () => mocks.app,
}));

describe("LayoutPersistence", () => {
	const storedValues = new Map<string, string>();

	beforeEach(() => {
		vi.useFakeTimers();
		storedValues.clear();
		vi.stubGlobal("localStorage", {
			getItem: (key: string) => storedValues.get(key) ?? null,
			setItem: (key: string, value: string) => storedValues.set(key, value),
			removeItem: (key: string) => storedValues.delete(key),
			clear: () => storedValues.clear(),
		});
		mocks.app.state = {
			...initialState,
			desks: initialState.desks.map((desk) => ({
				...desk,
				panes: [...desk.panes],
			})),
		};
		mocks.app.dispatch.mockReset();
		mocks.server.deskLayout = null;
		mocks.server.deskLayoutScope = null;
		mocks.server.session.desk.id = "control-desk-a";
		mocks.server.saveDeskLayout.mockReset().mockResolvedValue(undefined);
	});

	afterEach(() => {
		cleanup();
		vi.unstubAllGlobals();
		vi.useRealTimers();
	});

	it("never saves the fallback layout before the server has resolved the current show and user layout", async () => {
		const view = render(<LayoutPersistence />);
		await act(async () => vi.advanceTimersByTimeAsync(700));
		expect(mocks.server.saveDeskLayout).not.toHaveBeenCalled();

		mocks.server.deskLayoutScope = "show:user";
		view.rerender(<LayoutPersistence />);
		await act(async () => vi.advanceTimersByTimeAsync(700));
		expect(mocks.server.saveDeskLayout).not.toHaveBeenCalled();

		mocks.app.state = { ...mocks.app.state, activeDeskId: "operator-change" };
		view.rerender(<LayoutPersistence />);
		await act(async () => vi.advanceTimersByTimeAsync(700));
		expect(mocks.server.saveDeskLayout).toHaveBeenCalledTimes(1);
		expect(mocks.server.saveDeskLayout).toHaveBeenCalledWith(
			expect.objectContaining({ activeDeskId: "operator-change" }),
		);
	});

	it("hydrates a stored layout before allowing the hydrated state to be persisted", async () => {
		const storedDesks = [{ id: "tour", name: "Tour", panes: [] }];
		mocks.server.deskLayoutScope = "show:user";
		mocks.server.deskLayout = {
			revision: 7,
			body: { desks: storedDesks, activeDeskId: "tour" },
		};
		const view = render(<LayoutPersistence />);

		expect(mocks.app.dispatch).toHaveBeenCalledWith({
			type: "HYDRATE_LAYOUT",
			desks: storedDesks,
			activeDeskId: "tour",
			windowSettings: undefined,
		});
		await act(async () => vi.advanceTimersByTimeAsync(700));
		expect(mocks.server.saveDeskLayout).not.toHaveBeenCalled();

		mocks.app.state = {
			...mocks.app.state,
			desks: storedDesks,
			activeDeskId: "tour",
		};
		view.rerender(<LayoutPersistence />);
		await act(async () => vi.advanceTimersByTimeAsync(700));
		expect(mocks.server.saveDeskLayout).toHaveBeenCalledTimes(1);
		expect(mocks.server.saveDeskLayout).toHaveBeenCalledWith(
			expect.objectContaining({ desks: storedDesks, activeDeskId: "tour" }),
		);
	});

	it("restores built-in and pane modes only for the current real desk", () => {
		localStorage.setItem(
			"tosklight.fixture-sheet-compact-modes.v1:show:control-desk-a",
			JSON.stringify({
				builtIn: "text-only",
				desktops: {
					main: { "fixtures-left": "icon-only", "fixtures-right": "text-only" },
				},
			}),
		);
		mocks.server.deskLayoutScope = "show:user";
		const view = render(<LayoutPersistence />);

		expect(mocks.app.dispatch).toHaveBeenCalledWith({
			type: "HYDRATE_FIXTURE_SHEET_COMPACT_MODES",
			builtIn: "text-only",
			desktops: {
				main: { "fixtures-left": "icon-only", "fixtures-right": "text-only" },
			},
		});

		mocks.app.dispatch.mockClear();
		mocks.server.session.desk.id = "control-desk-b";
		view.rerender(<LayoutPersistence />);
		expect(mocks.app.dispatch).toHaveBeenCalledWith({
			type: "HYDRATE_FIXTURE_SHEET_COMPACT_MODES",
			builtIn: "off",
			desktops: {},
		});
	});

	it("persists compact-only changes locally without saving the portable show layout", async () => {
		const fixtureDesks = [
			{
				id: "main",
				name: "Main",
				panes: [
					{
						id: "fixtures-left",
						kind: "fixtures" as const,
						title: "Fixture Sheet",
						x: 1,
						y: 1,
						width: 12,
						height: 18,
					},
					{
						id: "fixtures-right",
						kind: "fixtures" as const,
						title: "Fixture Sheet",
						x: 13,
						y: 1,
						width: 12,
						height: 18,
					},
				],
			},
		];
		mocks.app.state = {
			...initialState,
			activeDeskId: "main",
			desks: fixtureDesks,
		};
		mocks.server.deskLayoutScope = "show:user";
		const view = render(<LayoutPersistence />);
		await act(async () => vi.advanceTimersByTimeAsync(700));

		mocks.app.state = {
			...mocks.app.state,
			fixtureSheetCompactMode: "text-only",
			desks: [
				{
					...fixtureDesks[0],
					panes: [
						{
							...fixtureDesks[0].panes[0],
							fixtureSheetCompactMode: "icon-only" as const,
						},
						{
							...fixtureDesks[0].panes[1],
							fixtureSheetCompactMode: "text-only" as const,
						},
					],
				},
			],
		};
		view.rerender(<LayoutPersistence />);
		await act(async () => vi.advanceTimersByTimeAsync(700));

		expect(mocks.server.saveDeskLayout).not.toHaveBeenCalled();
		expect(
			JSON.parse(
				localStorage.getItem(
					"tosklight.fixture-sheet-compact-modes.v1:show:control-desk-a",
				) ?? "null",
			),
		).toEqual({
			builtIn: "text-only",
			desktops: {
				main: { "fixtures-left": "icon-only", "fixtures-right": "text-only" },
			},
		});
	});

	it("strips compact modes from a server layout save triggered by another setting", async () => {
		mocks.server.deskLayoutScope = "show:user";
		mocks.app.state = {
			...initialState,
			fixtureSheetCompactMode: "icon-only",
			desks: initialState.desks.map((desktop) => ({
				...desktop,
				panes: desktop.panes.map((pane, index) =>
					index === 0
						? { ...pane, fixtureSheetCompactMode: "text-only" as const }
						: pane,
				),
			})),
		};
		const view = render(<LayoutPersistence />);
		await act(async () => vi.advanceTimersByTimeAsync(700));
		mocks.app.state = { ...mocks.app.state, playbackPage: 2 };
		view.rerender(<LayoutPersistence />);
		await act(async () => vi.advanceTimersByTimeAsync(700));

		const payload = mocks.server.saveDeskLayout.mock.calls[0]?.[0];
		expect(payload.windowSettings).not.toHaveProperty(
			"fixtureSheetCompactMode",
		);
		for (const desktop of payload.desks) {
			for (const pane of desktop.panes) {
				expect(pane).not.toHaveProperty("fixtureSheetCompactMode");
			}
		}
	});
});
