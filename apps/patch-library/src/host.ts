/**
 * Everything the patch sheet needs from the application that hosts it.
 *
 * The sheet itself owns patch semantics — addressing, conflicts, splits, placement, layers and
 * multi-patch. It owns none of the surrounding product: the lighting desk supplies a programmer
 * selection and arms editing from the `Set` key, while a planning application has no programmer
 * at all and edits directly. Both supply the same fixture library. Keeping that difference in one
 * injected port is what lets a single patch sheet serve both without either one importing the
 * other.
 */
import { createContext, useContext } from "react";
import type {
	FixtureDefinition,
	FixtureNote,
	FixtureProfile,
	FixtureVisibility,
	PatchLayer,
	VersionedObject,
} from "./wire";

/** The fixture library and patch layers the sheet lists, filters and patches from. */
export interface PatchLibraryHost {
	/** Legacy fixture definitions, merged with the transferable profiles for the browser. */
	fixtureLibrary: readonly FixtureDefinition[];
	fixtureProfiles: readonly FixtureProfile[];
	patchLayers: readonly VersionedObject<PatchLayer>[];
	fixtureVisibility?: ReadonlyMap<string, FixtureVisibility>;
	fixtureNotes?: ReadonlyMap<string, FixtureNote>;
	/**
	 * MVR fixtures that could not be resolved to a profile. Reported in the header so an import
	 * that silently dropped fixtures is visible; only the count is read.
	 */
	unresolvedMvrFixtures: readonly unknown[];
	savePatchLayer(layer: PatchLayer): Promise<boolean>;
	saveFixtureVisibility?(visibility: FixtureVisibility): Promise<boolean>;
	saveFixtureNote?(note: FixtureNote): Promise<boolean>;
}

/** One selection replacement. The sheet never builds any other selection intent. */
export interface PatchSelectionIntent {
	resolvedFixtures: readonly string[];
}

/**
 * The host's fixture selection, when it has one.
 *
 * A desk routes this to the shared programmer selection so the patch sheet, fixture sheet and
 * command line stay on the same fixtures. A host with no programmer supplies
 * {@link noPatchSelection}, and the sheet keeps its own row highlight only.
 */
export interface PatchSelectionHost {
	fixtureIds: ReadonlySet<string> | null;
	orderedFixtureIds: readonly string[] | null;
	replace(intent: PatchSelectionIntent): void;
}

/** A host with no programmer selection. Row clicks still move the sheet's own cursor. */
export const noPatchSelection: PatchSelectionHost = {
	fixtureIds: null,
	orderedFixtureIds: null,
	replace: () => undefined,
};

export interface PatchHost {
	library: PatchLibraryHost | null;
	selection: PatchSelectionHost;
	/**
	 * Whether inline cell editing is armed.
	 *
	 * On a desk this follows the `Set` key, so a stray tap on a live patch sheet cannot change an
	 * address mid-show. A planning application has no `Set` key and passes `true`.
	 */
	editArmed: boolean;
	/** Planning editors are continuously editable but row clicks still publish shared selection. */
	desktopEditing?: boolean;
	/**
	 * Called with `false` once an edit is committed or abandoned.
	 *
	 * A desk's `Set` key is one-shot: arming it authorizes one change, not a session. A host that
	 * is permanently armed ignores this.
	 */
	setEditArmed(armed: boolean): void;
}

const PatchHostContext = createContext<PatchHost | null>(null);

export const PatchHostProvider = PatchHostContext.Provider;

export function usePatchHost(): PatchHost {
	const host = useContext(PatchHostContext);
	if (!host)
		throw new Error("usePatchHost must be used inside a PatchHostProvider");
	return host;
}
