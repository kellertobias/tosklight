import { useEffect, useId, useRef, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import type { StagePaneGesture } from "../../platform/desktop/types";

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
	/** True while the native renderer is drawing this pane. */
	active: boolean;
	/** What went wrong, once the desk has something to say about it. */
	trouble: string | null;
	/** What is drawing, for the diagnostics an operator reads. */
	renderer: string | null;
	/** Forward a gesture the pane element captured. */
	send: (gesture: StagePaneGesture, x: number, y: number) => void;
}

/** How often the desk is asked what the pane is doing. Diagnostics, not the picture. */
const STATUS_INTERVAL = 2_000;

export function useNativeStagePane(enabled = true, live3d = true): NativeStagePane {
	const desktopBridge = useDesktopBridge();
	const paneId = useId();
	const [element, setElement] = useState<HTMLElement | null>(null);
	const [active, setActive] = useState(false);
	const [trouble, setTrouble] = useState<string | null>(null);
	const [renderer, setRenderer] = useState<string | null>(null);
	const [restart, setRestart] = useState(0);
	const opened = useRef(false);
	/** True once the desk has named a renderer, so a first poll cannot be read as a stop. */
	const drew = useRef(false);

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
		if (!enabled || !element || !available) return;
		let observer: ResizeObserver | null = null;
		let report: (() => void) | null = null;
		let unsubscribeMove: (() => void) | null = null;
		let disposed = false;

		const geometry = () => {
			const rect = element.getBoundingClientRect();
			const scale = window.devicePixelRatio || 1;
			return {
				x: rect.left,
				y: rect.top,
				width: rect.width,
				height: rect.height,
				scale,
				surfaceWidth: Math.max(1, Math.round(window.innerWidth * scale)),
				surfaceHeight: Math.max(1, Math.round(window.innerHeight * scale)),
			};
		};

		// Opening is started without awaiting anything first, so a re-render cannot cancel it
		// between a question and its answer.
		opened.current = true;
		void desktopBridge
			.openStagePane(paneId, live3d, geometry())
			.then(() => {
				setActive(true);
				report = () => {
					void desktopBridge.setStagePane(paneId, geometry());
				};
				observer = new ResizeObserver(report);
				observer.observe(element);
				window.addEventListener("resize", report);
				window.addEventListener("scroll", report, true);
				void desktopBridge.onCurrentWindowMoved(report).then((unsubscribe) => {
					if (disposed) unsubscribe();
					else unsubscribeMove = unsubscribe;
				});
				report();
			})
			.catch((error) => {
				// The desk could not start or attach the renderer. The caller keeps its web
				// renderer, and the reason is shown rather than swallowed.
				opened.current = false;
				setTrouble(String(error));
			});

		return () => {
			disposed = true;
			observer?.disconnect();
			unsubscribeMove?.();
			if (report) {
				window.removeEventListener("resize", report);
				window.removeEventListener("scroll", report, true);
			}
			if (opened.current) {
				opened.current = false;
				drew.current = false;
				setActive(false);
				void desktopBridge.closeStagePane(paneId);
			}
		};
	}, [element, enabled, available, desktopBridge, paneId, live3d, restart]);

	useEffect(() => {
		if (!active) return;
		let cancelled = false;
		const poll = async () => {
			const [description, detail] = await desktopBridge.stagePaneStatus(paneId);
			if (cancelled) return;
			setRenderer(description);
			setTrouble(detail);
			// The desk takes the pane down when its renderer dies rather than holding a still
			// picture of a rig that has since moved. Noticing that here is what returns the Stage
			// to the web renderer instead of leaving a transparent hole where it was.
			//
			// Only after the desk has named a renderer at least once: the first poll can land
			// before the desk has finished opening, and reading that as a stop would have the
			// interface tear down a pane the desk is still holding.
			if (description !== null) drew.current = true;
			else if (drew.current) {
				setActive(false);
				setRestart((value) => value + 1);
			}
		};
		void poll();
		const timer = window.setInterval(() => void poll(), STATUS_INTERVAL);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [active, desktopBridge, paneId]);

	return {
		id: paneId,
		ref: setElement,
		active,
		trouble,
		renderer,
		send: (gesture, x, y) => {
			if (!active) return;
			void desktopBridge.sendStagePaneInput(gesture, x, y, paneId);
		},
	};
}
