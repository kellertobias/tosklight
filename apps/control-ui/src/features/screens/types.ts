import type {
	BootstrapSnapshot,
	ControlDesk,
	PlaybackPage,
	PlaybackSnapshot,
	ScreenConfiguration,
	ScreenSnapshot,
	SessionResponse,
} from "../../api/types";

export interface ScreenCapabilities {
	screens: ScreenSnapshot | null;
	saveScreen: (screen: ScreenConfiguration) => Promise<void>;
	deleteScreen: (id: string) => Promise<void>;
	setScreenPage: (id: string, page: number) => Promise<void>;
}

export interface ScreensContextValue extends ScreenCapabilities {
	bootstrap: BootstrapSnapshot | null;
	session: SessionResponse | null;
	playbacks: PlaybackSnapshot | null;
	savePlaybackPage: (page: PlaybackPage) => Promise<boolean>;
	updateControlDesk: (desk: ControlDesk) => Promise<void>;
	selectControlDesk: (id: string) => void;
	removeClient: (deskId: string) => Promise<boolean>;
}
