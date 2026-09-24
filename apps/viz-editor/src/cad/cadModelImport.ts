/**
 * Loading a 3D model file into the show as a Venue object: the native file picker, then the import.
 *
 * The show carries the imported model itself, so it opens again with the show; a file the importer
 * cannot read is refused with its reason and leaves the show as it was. The model lands at the
 * stage origin, placed, turned and scaled from Info like any other Venue object.
 */
import { open } from "@tauri-apps/plugin-dialog";
import { documentSession } from "../document/session";
import { VENUE_MODEL_EXTENSIONS } from "./venueModelFormats";

/** What the add buttons pass to mean "load a model file" rather than a part from the library. */
export const LOAD_MODEL = "load-model";

/**
 * Asks for a model file and imports it; null when the operator closes the picker. `onImporting` is
 * told once a file is chosen, so progress shows while the import runs rather than while choosing.
 */
export async function chooseAndImportModel(
	onImporting?: () => void,
): Promise<{ fixtureId: string; name: string } | null> {
	const path = await open({
		multiple: false,
		directory: false,
		filters: [{ name: "3D model", extensions: [...VENUE_MODEL_EXTENSIONS] }],
	});
	if (typeof path !== "string") return null;
	onImporting?.();
	return documentSession.importVenueModel(path, null);
}
