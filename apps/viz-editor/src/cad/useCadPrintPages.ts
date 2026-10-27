/** Print pages travel in the native show; only legacy adoption bookkeeping stays on this machine. */
import { useEffect, useRef, useState } from "react";
import { printPageSession, type PrintPageSnapshot } from "./printPages";
import { rotatePrintPage } from "./print";
import { changePrintPage } from "./printLayout";
import {
	type CadPrintPage,
	legacyTopDownPlanPoint,
	type ViewportTile,
} from "./types";

const PRINT_KEY = "tosklight:viz-editor:cad-print-pages:v2";
const ADOPTED_KEY = "tosklight:viz-editor:cad-print-pages:adopted-show:v1";
const LEGACY_PRINT_KEY = "tosklight:viz-editor:cad-print-pages:v1";

function pageId(prefix: string, pageNumber: number): string {
	return (
		globalThis.crypto?.randomUUID?.() ?? `${prefix}-${Date.now()}-${pageNumber}`
	);
}

/**
 * The pages a previous session left behind.
 *
 * A page is kept only when it still carries the fields a sheet needs; everything optional is
 * normalised here so the rest of the screen never has to ask what an older file meant. v1 stored
 * top-down plans mirrored, and is converted once on first read.
 */
export function restorePrintPages(): CadPrintPage[] {
	try {
		const current = localStorage.getItem(PRINT_KEY);
		const legacy = current == null;
		const stored = JSON.parse(
			current ?? localStorage.getItem(LEGACY_PRINT_KEY) ?? "[]",
		);
		if (!Array.isArray(stored)) return [];
		return stored
			.filter(
				(page): page is CadPrintPage =>
					typeof page?.id === "string" &&
					typeof page?.tileId === "string" &&
					typeof page?.name === "string" &&
					page?.centreMillimetres?.length === 2 &&
					Number.isFinite(page?.widthMillimetres),
			)
			.map((page) => ({
				...page,
				rotationQuarterTurns: page.rotationQuarterTurns ?? 0,
				centreMillimetres:
					legacy && page.view === "top_down"
						? legacyTopDownPlanPoint(
								page.centreMillimetres,
								page.rotationQuarterTurns ?? 0,
							)
						: page.centreMillimetres,
				kind: page.kind === "fixture_list" ? "fixture_list" : "plan",
				orientation: page.orientation === "portrait" ? "portrait" : "landscape",
				showFixtureIds: page.showFixtureIds === true,
				showDmxAddresses: page.showDmxAddresses === true,
				showMountingHardware: page.showMountingHardware !== false,
			}));
	} catch {
		return [];
	}
}

/** Claim the old workspace for one show before asynchronous work starts. A failed write can
 * retry for that show, but the pages can never be adopted by a different show afterward. */
export function legacyPagesForShow(showId: string): CadPrintPage[] {
	const owner = localStorage.getItem(ADOPTED_KEY);
	if (owner !== null && owner !== showId) return [];
	localStorage.setItem(ADOPTED_KEY, showId);
	return restorePrintPages();
}

/** Rebase only fields this window changed, keeping other windows' page edits and additions. */
export function rebasePrintPages(base: CadPrintPage[], local: CadPrintPage[], remote: CadPrintPage[]): CadPrintPage[] {
	const old = new Map(base.map((page) => [page.id, page]));
	const own = new Map(local.map((page) => [page.id, page]));
	const merged = remote.filter((page) => !old.has(page.id) || own.has(page.id)).map((page) => {
		const edited = own.get(page.id);
		const original = old.get(page.id);
		if (!edited || !original) return edited ?? page;
		const result = { ...page };
		for (const key of new Set([...Object.keys(original), ...Object.keys(edited)])) {
			const field = key as keyof CadPrintPage;
			if (JSON.stringify(original[field]) !== JSON.stringify(edited[field])) {
				Object.assign(result, { [field]: edited[field] });
			}
		}
		return result;
	});
	const ids = new Set(merged.map((page) => page.id));
	for (const page of local) {
		if (!ids.has(page.id) && (!old.has(page.id) || JSON.stringify(old.get(page.id)) !== JSON.stringify(page))) merged.push(page);
	}
	return merged.map((page) => changePrintPage(page, {}));
}

export interface CadPrintPages {
	pages: CadPrintPage[];
	selectedId: string | null;
	busy: boolean;
	ready: boolean;
	error: string | null;
	clearError(): void;
	retry(): void;
	select(id: string | null): void;
	/** A plan page framing what a tile currently shows, including its depth slice. */
	addPlanPage(tile: ViewportTile): void;
	addFixtureList(): void;
	change(id: string, change: Partial<CadPrintPage>): void;
	rotate(id: string): void;
	remove(id: string): void;
}

type PrintPageState = { showId: string | null; pages: CadPrintPage[] };
type PrintPageWriter = {
	showId: string;
	update(pages: CadPrintPage[]): void;
	retry(): Promise<void>;
};

interface PrintPagePersistenceBindings {
	currentPages: { current: CadPrintPage[] };
	writer: { current: PrintPageWriter | null };
	setState(state: PrintPageState): void;
	setBusy(busy: boolean): void;
	setError(error: string | null): void;
	setReadyShow(showId: string | null): void;
}

/** Connect one loaded show to its native page store until the hook changes shows or unmounts. */
function connectPrintPagePersistence(
	currentShow: string,
	bindings: PrintPagePersistenceBindings,
) {
	const { currentPages, writer, setState, setBusy, setError, setReadyShow } = bindings;
	let disposed = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let unlisten: (() => void) | undefined;
	let pending: CadPrintPage[] | undefined;
	let saving = false;
	let dirty = false;
	let failed = false;
	let revision = 0;
	let deferred: PrintPageSnapshot | undefined;
	let baseline: CadPrintPage[] = [];
	const fail = (reason: unknown) => {
		if (!disposed) setError(`Print pages could not be saved or loaded: ${String(reason)}`);
	};
	function accept(snapshot: PrintPageSnapshot) {
		revision = snapshot.revision;
		baseline = (snapshot.pages ?? []).map((page) => changePrintPage(page, {}));
		currentPages.current = baseline;
		setState({ showId: currentShow, pages: baseline });
	}
	async function flush() {
		if (saving || !pending || failed) return;
		saving = true;
		const next = pending;
		pending = undefined;
		try {
			const saved = await printPageSession.save(currentShow, next, false, revision);
			revision = saved.revision;
			baseline = next;
			dirty = pending !== undefined;
			if (!disposed) setError(null);
		} catch (reason) {
			failed = true;
			fail(reason);
		} finally {
			saving = false;
			if (!dirty && deferred && deferred.revision > revision && !disposed) accept(deferred);
			if (deferred && deferred.revision <= revision) deferred = undefined;
			if (pending && !failed) void flush();
			else if (!disposed) setBusy(false);
		}
	}
	async function retry() {
		if (saving) return;
		clearTimeout(timer);
		setBusy(true);
		try {
			const latest = await printPageSession.all(currentShow);
			if (disposed) return;
			const rebased = rebasePrintPages(baseline, currentPages.current, latest.pages ?? []);
			revision = latest.revision;
			baseline = (latest.pages ?? []).map((page) => changePrintPage(page, {}));
			currentPages.current = rebased;
			setState({ showId: currentShow, pages: rebased });
			pending = rebased;
			failed = false;
			await flush();
		} catch (reason) { fail(reason); setBusy(false); }
	}
	async function load() {
		try {
			const stop = await printPageSession.onDelta((snapshot) => {
				if (disposed || snapshot.showId !== currentShow || snapshot.revision <= revision) return;
				if (dirty || saving) {
					if (!deferred || snapshot.revision > deferred.revision) deferred = snapshot;
					return;
				}
				accept(snapshot);
			});
			if (disposed) { stop(); return; }
			unlisten = stop;
			const loaded = await printPageSession.all(currentShow);
			if (disposed) return;
			const legacy = legacyPagesForShow(currentShow);
			const restored = loaded.pages === null
				? await printPageSession.save(currentShow, legacy, true, loaded.revision)
				: loaded;
			if (disposed) return;
			if (localStorage.getItem(ADOPTED_KEY) === currentShow) {
				localStorage.removeItem(PRINT_KEY);
				localStorage.removeItem(LEGACY_PRINT_KEY);
			}
			if (restored.revision >= revision) accept(restored);
			writer.current = {
				showId: currentShow,
				retry,
				update(next) {
					pending = next;
					dirty = true;
					// A conflict remains visible until Retry rebases against the latest show.
					if (failed) return;
					setError(null);
					setBusy(true);
					clearTimeout(timer);
					timer = setTimeout(() => void flush(), 250);
				},
			};
			setReadyShow(currentShow);
		} catch (reason) { fail(reason); }
		finally { if (!disposed) setBusy(false); }
	}
	void load();
	return () => {
		disposed = true;
		unlisten?.();
		clearTimeout(timer);
		void flush();
	};
}

export function useCadPrintPages(showId: string | null): CadPrintPages {
	const [state, setState] = useState<PrintPageState>({ showId: null, pages: [] });
	const currentPages = useRef<CadPrintPage[]>([]);
	const [selectedId, setSelectedId] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [readyShow, setReadyShow] = useState<string | null>(null);
	const [attempt, setAttempt] = useState(0);
	const [error, setError] = useState<string | null>(null);
	const writer = useRef<PrintPageWriter | null>(null);
	const pages = state.showId === showId ? state.pages : [];

	useEffect(() => {
		writer.current = null;
		setReadyShow(null);
		setSelectedId(null);
		setState({ showId, pages: [] });
		currentPages.current = [];
		setError(null);
		setBusy(showId !== null);
		if (showId === null) return;
		return connectPrintPagePersistence(showId, {
			currentPages, writer, setState, setBusy, setError, setReadyShow,
		});
	}, [showId, attempt]);

	function setPages(change: (current: CadPrintPage[]) => CadPrintPage[]) {
		if (writer.current?.showId !== showId || state.showId !== showId) return;
		const next = change(currentPages.current);
		currentPages.current = next;
		writer.current.update(next);
		setState({ showId, pages: next });
	}
	function add(page: CadPrintPage) {
		setPages((current) => [...current, page]);
		setSelectedId(page.id);
	}

	return {
		pages,
		selectedId,
		busy,
		ready: showId !== null && readyShow === showId,
		error,
		clearError: () => setError(null),
		retry() {
			if (writer.current?.showId === showId) void writer.current.retry();
			else setAttempt((value) => value + 1);
		},
		select: setSelectedId,
		addPlanPage(tile) {
			const pageNumber = pages.length + 1;
			add({
				kind: "plan",
				id: pageId("page", pageNumber),
				tileId: tile.id,
				name: `Page ${pageNumber}`,
				view: tile.view,
				rotationQuarterTurns: tile.rotationQuarterTurns,
				cutPlanes: tile.cutPlanes,
				centreMillimetres: [-tile.camera.pan[0], -tile.camera.pan[1]],
				widthMillimetres: Math.max(3000, 360 / tile.camera.zoom),
				included: true,
				orientation: "landscape",
				showFixtureIds: false,
				showDmxAddresses: false,
				showMountingHardware: true,
			});
		},
		addFixtureList() {
			const pageNumber = pages.length + 1;
			add({
				kind: "fixture_list",
				id: pageId("fixture-list", pageNumber),
				tileId: "fixture-list",
				name: "Fixture List",
				view: "top_down",
				rotationQuarterTurns: 0,
				centreMillimetres: [0, 0],
				widthMillimetres: 2970,
				included: true,
				orientation: "landscape",
				showFixtureIds: true,
				showDmxAddresses: true,
				showMountingHardware: true,
			});
		},
		change(id, change) {
			setPages((current) =>
				current.map((page) => (page.id === id ? changePrintPage(page, change) : page)),
			);
		},
		remove(id) {
			const existing = currentPages.current;
			const index = existing.findIndex((page) => page.id === id);
			if (index < 0 || writer.current?.showId !== showId) return;
			const remaining = existing.filter((page) => page.id !== id);
			setPages(() => remaining);
			setSelectedId((selected) => selected === id ? remaining[Math.min(index, remaining.length - 1)]?.id ?? null : selected);
		},

		rotate(id) {
			setPages((current) =>
				current.map((page) => (page.id === id ? rotatePrintPage(page) : page)),
			);
		},
	};
}
