import type { ScreenConfiguration } from "../../api/types";
import { useScreens } from "./ScreensContext";

/**
 * Encoder placement is independent of Playback controls: it only decides which surface
 * carries the encoder section. The persisted field keeps its `owner_screen_id` name so
 * existing desk data stays readable; `null` means the main screen.
 */
export type EncoderPlacement = {
	/** `null` while the encoders belong to the main screen. */
	encoderScreenId: string | null;
	/** True while this surface renders the encoder section. */
	holdsEncoders: boolean;
	/** The configured screen, when the encoders live on a secondary screen. */
	encoderScreen: ScreenConfiguration | null;
	visibleEncoders: number;
};

export function useEncoderPlacement(
	screenId: string | null = null,
): EncoderPlacement | null {
	const { screens } = useScreens();
	const configuration = screens?.programmer_control_surface;
	if (!configuration) return null;
	const encoderScreenId = configuration.owner_screen_id ?? null;
	return {
		encoderScreenId,
		holdsEncoders: encoderScreenId === screenId,
		encoderScreen:
			screens?.screens.find((screen) => screen.id === encoderScreenId) ?? null,
		visibleEncoders: configuration.visible_encoders,
	};
}

/** A secondary screen offers the Playback/Encoders switch only when it carries both. */
export function screenShowsPlaybacks(screen: ScreenConfiguration) {
	return Boolean(screen.show_playbacks || screen.show_page_controls);
}
