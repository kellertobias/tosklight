import { useEffect, useRef } from "react";
import type { ScreenConfiguration } from "../../api/types";
import { useScreens } from "../../features/screens/ScreensContext";
import { type DesktopBridge, useDesktopBridge } from "../../platform/desktop";

type ReconcileState = Map<string, string>;

function screenSignature(screen: ScreenConfiguration, available: boolean) {
	return JSON.stringify({
		desiredOpen: screen.desired_open,
		available,
		title: screen.name,
		displayId: screen.display_id,
		bounds: screen.bounds,
		fullscreen: screen.fullscreen,
	});
}

async function reconcileScreen(
	desktop: DesktopBridge,
	screen: ScreenConfiguration,
	displays: ReadonlySet<string>,
	state: ReconcileState,
) {
	const available = !screen.display_id || displays.has(screen.display_id);
	const signature = screenSignature(screen, available);
	if (state.get(screen.id) === signature) return;
	if (!screen.desired_open) await desktop.closeConsoleScreen(screen.id);
	else if (!available) await desktop.hideConsoleScreen(screen.id);
	else
		await desktop.openConsoleScreen({
			screenId: screen.id,
			title: screen.name,
			displayId: screen.display_id,
			bounds: screen.bounds,
			fullscreen: screen.fullscreen,
		});
	state.set(screen.id, signature);
}

export async function reconcileScreenWindows(
	desktop: DesktopBridge,
	screens: readonly ScreenConfiguration[],
	state: ReconcileState,
	cancelled: () => boolean,
) {
	const available = new Set(
		(await desktop.listDisplays()).map((display) => display.id),
	);
	const configured = new Set(screens.map((screen) => screen.id));
	for (const screenId of state.keys()) {
		if (!configured.has(screenId)) {
			await desktop.closeConsoleScreen(screenId);
			state.delete(screenId);
		}
	}
	for (const screen of screens) {
		if (cancelled()) return;
		await reconcileScreen(desktop, screen, available, state);
	}
}

function createReconciler(
	desktop: DesktopBridge,
	screens: () => readonly ScreenConfiguration[],
	cancelled: () => boolean,
) {
	let running = false;
	let requested = false;
	const state: ReconcileState = new Map();
	return async function request() {
		requested = true;
		if (running) return;
		running = true;
		try {
			while (requested && !cancelled()) {
				requested = false;
				await reconcileScreenWindows(desktop, screens(), state, cancelled);
			}
		} finally {
			running = false;
		}
	};
}

export function ScreenWindowManager() {
	const desktop = useDesktopBridge();
	const screens = useScreens().screens;
	const screensRef = useRef(screens);
	const requestReconcile = useRef<() => void>(() => undefined);
	screensRef.current = screens;
	useEffect(() => {
		if (!desktop.available) return;
		let cancelled = false;
		const request = createReconciler(
			desktop,
			() => screensRef.current?.screens ?? [],
			() => cancelled,
		);
		requestReconcile.current = () => void request();
		requestReconcile.current();
		const timer = window.setInterval(requestReconcile.current, 2_000);
		return () => {
			cancelled = true;
			requestReconcile.current = () => undefined;
			window.clearInterval(timer);
		};
	}, [desktop]);
	useEffect(() => requestReconcile.current(), [screens]);
	return null;
}
