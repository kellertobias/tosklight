import { type MutableRefObject, useEffect, useRef } from "react";
import type { ScreenConfiguration } from "../../api/types";
import { useDesktopBridge } from "./DesktopContext";
import type { DesktopBridge, DesktopUnsubscribe } from "./types";

type SaveScreen = (screen: ScreenConfiguration) => Promise<void>;

class ScreenWindowPersistence {
	private timer: number | undefined;
	private shuttingDown = false;
	private disposed = false;
	private unsubscribes: DesktopUnsubscribe[] = [];

	constructor(
		private readonly desktop: DesktopBridge,
		private readonly screen: MutableRefObject<ScreenConfiguration | undefined>,
		private readonly closing: MutableRefObject<boolean>,
		private readonly save: SaveScreen,
	) {}

	request = () => {
		window.clearTimeout(this.timer);
		if (this.closing.current || this.disposed) return;
		this.timer = window.setTimeout(() => void this.persist(), 300);
	};

	private async persist() {
		if (this.closing.current || this.disposed) return;
		const state = await this.desktop.currentWindowState();
		const screen = this.screen.current;
		if (!screen || this.closing.current || this.disposed) return;
		await this.save({
			...screen,
			display_id: state.displayId ?? screen.display_id,
			bounds: state.bounds,
			fullscreen: state.fullscreen,
		});
	}

	private close = async () => {
		this.closing.current = true;
		window.clearTimeout(this.timer);
		const screen = this.screen.current;
		if (!this.shuttingDown && screen)
			await this.save({ ...screen, desired_open: false });
		await this.desktop.destroyCurrentWindow();
	};

	async subscribe() {
		this.unsubscribes = await Promise.all([
			this.desktop.onApplicationShuttingDown(() => {
				this.shuttingDown = true;
			}),
			this.desktop.onCurrentWindowMoved(this.request),
			this.desktop.onCurrentWindowResized(this.request),
			this.desktop.onCurrentWindowCloseRequested(this.close),
		]);
		if (this.disposed) this.unsubscribe();
		else this.request();
	}

	dispose() {
		this.disposed = true;
		window.clearTimeout(this.timer);
		this.unsubscribe();
	}

	private unsubscribe() {
		for (const unsubscribe of this.unsubscribes) unsubscribe();
		this.unsubscribes = [];
	}
}

export function useScreenWindowPersistence(
	screen: ScreenConfiguration | undefined,
	save: SaveScreen,
) {
	const desktop = useDesktopBridge();
	const screenRef = useRef(screen);
	const closing = useRef(false);
	screenRef.current = screen;
	useEffect(() => {
		if (!screen || !desktop.available) return;
		const persistence = new ScreenWindowPersistence(
			desktop,
			screenRef,
			closing,
			save,
		);
		void persistence.subscribe();
		return () => persistence.dispose();
	}, [screen?.id, desktop, save]);
	return closing;
}
