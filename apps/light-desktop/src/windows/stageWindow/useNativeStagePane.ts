import { useCallback, useEffect, useId, useRef, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import type { StagePaneGesture } from "../../platform/desktop/types";
import { NativeStageAttachment } from "./nativeStageAttachment";
import { observeNativeStageVisibility } from "./nativeStageVisibility";

/**
 * The Stage drawn by the native renderer, in a rectangle of the desk's own window.
 *
 * The desk's window is a native window with the interface added on top as a transparent child, so
 * there is a surface underneath everything drawn here. This hook owns the arrangement from the web
 * side: it reports where the pane element is, and the desk draws the picture there, beneath the
 * interface. Where the element is, is the only thing the web layer decides — the layout owns the
 * geometry because the pane is an element in a layout, and the renderer is told.
 *
 * It is not always possible, and that is ordinary. A browser has no second process; a platform may
 * have no way to move a picture between two of them; an installation may be missing its renderer.
 * In every one of those the hook stays inactive and the caller keeps the web renderer, which is
 * why `active` is the answer rather than an error.
 */
export interface NativeStagePane {
	/** Stable identity for routing this pane within its native window. */
	id: string;
	/** Attach to the element the pane should fill. */
	ref: (element: HTMLElement | null) => void;
	/** True while the native attachment exists, including when its picture is suspended. */
	active: boolean;
	/** What went wrong, once the desk has something to say about it. */
	trouble: string | null;
	/** What is drawing, for the diagnostics an operator reads. */
	renderer: string | null;
	/** Surface bounded command failures in the same pane status as native failures. */
	reportError?: (error: unknown) => void;
	/** Forward a gesture the pane element captured. */
	send: (gesture: StagePaneGesture, x: number, y: number) => void;
}

/** How often the desk is asked what the pane is doing. Diagnostics, not the picture. */
const STATUS_INTERVAL = 2_000;
const MAX_RESTARTS = 5;
const HEALTHY_RESET_MILLIS = 30_000;

export function useNativeStagePane(
	enabled = true,
	live3d = true,
): NativeStagePane {
	const desktopBridge = useDesktopBridge();
	const paneId = useId();
	const [element, setElement] = useState<HTMLElement | null>(null);
	const [active, setActive] = useState(false);
	const [trouble, setTrouble] = useState<string | null>(null);
	const reportError = useCallback((error: unknown) => setTrouble(String(error)), []);
	const [renderer, setRenderer] = useState<string | null>(null);
	const [restart, setRestart] = useState(0);
	const [retryDelay, setRetryDelay] = useState<number | null>(null);
	const recovery = useRef({ attempts: 0, healthySince: null as number | null });
	const [attachRequested, setAttachRequested] = useState(enabled);
	const enabledRef = useRef(enabled);
	enabledRef.current = enabled;
	const operations = useRef(Promise.resolve());
	const refreshVisibility = useRef<(() => void) | null>(null);
	useEffect(() => {
		if (enabled) setAttachRequested(true);
		refreshVisibility.current?.();
	}, [enabled]);
	useEffect(() => {
		if (retryDelay === null || !enabled) return;
		const timer = window.setTimeout(() => {
			setRetryDelay(null);
			setRestart((value) => value + 1);
		}, retryDelay);
		return () => window.clearTimeout(timer);
	}, [retryDelay, enabled]);

	/*
	 * Reported from the element rather than from any layout constant. The pane moves when a sheet
	 * opens beside it, when the window resizes, when the operator changes the desk layout — and
	 * none of those tell this hook anything. Measuring the element covers all of them without
	 * either side knowing the other's sizing rules.
	 */
	/*
	 * Whether the desk can embed at all, asked once.
	 *
	 * Deliberately separate from opening. Asking is asynchronous, and when the two were one effect
	 * every re-render tore down the pending question before it was answered — so the answer arrived
	 * to a run that had already been cancelled, and the pane was never opened at all.
	 */
	const [available, setAvailable] = useState<boolean | null>(null);
	useEffect(() => {
		if (!desktopBridge.available) {
			setAvailable(false);
			return;
		}
		let cancelled = false;
		void desktopBridge
			.stagePaneAvailable()
			.then((answer) => {
				if (!cancelled) setAvailable(answer);
			})
			.catch(() => {
				if (!cancelled) setAvailable(false);
			});
		return () => {
			cancelled = true;
		};
	}, [desktopBridge]);

	useEffect(() => {
		if (!attachRequested || !element || !available) return;
		let disposed = false;
		let unsubscribeMove: (() => void) | null = null;
		const serialize = (work: () => Promise<void>) => {
			const operation = operations.current.then(work);
			operations.current = operation.catch(() => undefined);
			return operation;
		};
		const attachment = new NativeStageAttachment(desktopBridge, paneId, live3d, serialize,
			() => { if (!disposed) setActive(true); },
			(error) => { if (!disposed) setTrouble(String(error)); });
		const visibility = observeNativeStageVisibility(element, () => enabledRef.current,
			(geometry, visible) => attachment.update(geometry, visible));
		refreshVisibility.current = visibility.report;
		void desktopBridge.onCurrentWindowMoved(visibility.report).then((unsubscribe) => {
			if (disposed) unsubscribe(); else unsubscribeMove = unsubscribe;
		}).catch((error) => { if (!disposed) setTrouble(String(error)); });
		return () => {
			disposed = true;
			visibility.dispose();
			if (refreshVisibility.current === visibility.report) refreshVisibility.current = null;
			unsubscribeMove?.();
			setActive(false);
			attachment.dispose();
		};
	}, [element, attachRequested, available, desktopBridge, paneId, live3d, restart]);

	useEffect(() => {
		// Active is set only after the serialized native open and initial visibility update finish.
		// A missing description now means loss, including death before the first status response.
		if (!active) return;
		let cancelled = false;
		let lost = false;
		let inFlight = false;
		const poll = async () => {
			if (inFlight || lost) return;
			inFlight = true;
			try {
				const [description, detail] = await desktopBridge.stagePaneStatus(paneId);
				if (cancelled || lost) return;
				setRenderer(description);
				setTrouble(detail);
				if (description !== null) {
					recovery.current.healthySince ??= Date.now();
					if (Date.now() - recovery.current.healthySince >= HEALTHY_RESET_MILLIS)
						recovery.current.attempts = 0;
					return;
				}
				lost = true;
				setActive(false);
				recovery.current.healthySince = null;
				if (recovery.current.attempts >= MAX_RESTARTS) {
					setTrouble(`${detail ?? "Stage renderer stopped repeatedly"}; reopen the Stage to try again.`);
					return;
				}
				setRetryDelay(Math.min(4_000, 250 * 2 ** recovery.current.attempts++));
			} catch (error) {
				if (!cancelled) setTrouble(String(error));
			} finally { inFlight = false; }
		};
		void poll();
		const timer = window.setInterval(() => void poll(), STATUS_INTERVAL);
		return () => { cancelled = true; window.clearInterval(timer); };
	}, [active, desktopBridge, paneId]);

	return {
		id: paneId,
		ref: setElement,
		active,
		trouble,
		renderer,
		reportError,
		send: (gesture, x, y) => {
			if (!active || !enabledRef.current) return;
			void Promise.resolve(desktopBridge.sendStagePaneInput(gesture, x, y, paneId)).catch(reportError);
		},
	};
}
