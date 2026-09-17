import type { ScreenAttachment } from "../../api/client/screenAttachment";
import type {
	BootstrapSnapshot,
	ControlDesk,
	ProgrammerControlSurfacePatch,
	ScreenConfiguration,
	ScreenSnapshot,
	SessionResponse,
} from "../../api/types";

export interface ScreenCapabilities {
	screens: ScreenSnapshot | null;
	saveScreen: (screen: ScreenConfiguration) => Promise<void>;
	deleteScreen: (id: string) => Promise<void>;
	setScreenPage: (id: string, page: number) => Promise<void>;
	updateProgrammerControlSurface: (
		patch: ProgrammerControlSurfacePatch,
	) => Promise<void>;
}

export interface ScreensContextValue extends ScreenCapabilities {
	bootstrap: BootstrapSnapshot | null;
	session: SessionResponse | null;
	/** What a screen window opened by this desk needs to join the same server and session. */
	screenAttachment?: ScreenAttachment | null;
	updateControlDesk: (desk: ControlDesk, options?: { throwOnError?: boolean; hardwareLighting?: import("../../api/types/desk").HardwareLightingPatch }) => Promise<void>;
	selectControlDesk: (id: string) => void;
	removeClient: (deskId: string, clientId: string) => Promise<boolean>;
}
