import type { ScreenConfiguration } from "../../api/types";

export type DesktopUnsubscribe = () => void;

export interface DesktopDisplay {
	id: string;
	name: string;
}

export interface DesktopWindowState {
	displayId: string | null;
	bounds: NonNullable<ScreenConfiguration["bounds"]>;
	fullscreen: boolean;
}

export interface ConsoleScreenWindow {
	screenId: string;
	title: string;
	displayId: string | null;
	bounds: ScreenConfiguration["bounds"];
	fullscreen: boolean;
}

export interface DesktopBridge {
	readonly available: boolean;
	frontendReady(): Promise<void>;
	exitApplication(): Promise<void>;
	cancelQuit(): Promise<void>;
	onQuitRequested(handler: () => void): Promise<DesktopUnsubscribe>;
	onApplicationShuttingDown(handler: () => void): Promise<DesktopUnsubscribe>;
	listDisplays(): Promise<DesktopDisplay[]>;
	openConsoleScreen(screen: ConsoleScreenWindow): Promise<void>;
	hideConsoleScreen(screenId: string): Promise<void>;
	closeConsoleScreen(screenId: string): Promise<void>;
	currentWindowState(): Promise<DesktopWindowState>;
	currentWindowFullscreen(): Promise<boolean>;
	setCurrentWindowFullscreen(fullscreen: boolean): Promise<void>;
	closeCurrentWindow(): Promise<void>;
	destroyCurrentWindow(): Promise<void>;
	startCurrentWindowDrag(): Promise<void>;
	onCurrentWindowMoved(handler: () => void): Promise<DesktopUnsubscribe>;
	onCurrentWindowResized(handler: () => void): Promise<DesktopUnsubscribe>;
	onCurrentWindowCloseRequested(
		handler: () => void | Promise<void>,
	): Promise<DesktopUnsubscribe>;
}
