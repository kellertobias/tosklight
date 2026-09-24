/**
 * What the CAD screen's toolbar offers every viewport: adding venue objects from the fixture
 * library, and the tool the pointer draws with.
 *
 * The toolbar and the tiles are siblings deep inside the CAD screen, so they meet here rather than
 * through every layer between. Without a provider there is no toolbar and the pointer only selects.
 */
import {
	createContext,
	type ReactNode,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";
import { type CadAnnotation, annotationSession } from "./annotations";
import { type PlacedWith, placeProfile } from "./cadPlacement";

/**
 * What the add buttons place: a truss, a stage element, a curtain, a primitive shape (box, cylinder
 * or ball), or any other Venue object.
 */
export type CadAddKind = "truss" | "stage" | "curtain" | "primitive" | "venue";

/** What a press on a viewport does. */
export type CadDrawTool = "select" | "polyline" | "box" | "text" | "measure" | "erase";

export interface CadTools {
	/**
	 * Adds a kind of object: the named profile, or else the part the button places now (a Venue
	 * element opens its picture list). With `several`, the part's bulk wizard opens instead of one
	 * being placed. Null hides the toolbar.
	 */
	onAdd: ((kind: CadAddKind, profileId?: string, several?: boolean) => void) | null;
	tool: CadDrawTool;
	setTool(tool: CadDrawTool): void;
	annotations: readonly CadAnnotation[];
	save(annotation: CadAnnotation): Promise<void>;
	remove(id: string): Promise<void>;
	/** Changes an item already drawn — moves or rewords text — as one step Undo puts back. */
	change(annotation: CadAnnotation): Promise<void>;
	/**
	 * The drawn text the Select tool has picked, apart from the rig's selection: picking text
	 * leaves the geometry under it alone, and picking an element puts the text down.
	 */
	selectedTextId: string | null;
	selectText(id: string | null): void;
	/** Where a text being dragged is drawn until the move is committed. */
	textPreview: { id: string; points: [number, number][] } | null;
	setTextPreview(preview: { id: string; points: [number, number][] } | null): void;
	/** Why the show refused the last drawn item, until the operator dismisses it. */
	error: string | null;
	clearError(): void;
	/** What an add button placed last, so the CAD screen can select it and open Info. */
	placed: CadPlaced | null;
	/** Tells the CAD screen an add flow has just placed these objects. */
	announcePlaced(fixtureIds: string | readonly string[]): void;
	/**
	 * The Venue element **Add Several** is placing: every press on a viewport places one more copy
	 * where it lands, until Escape or another tool ends it.
	 */
	placing: CadPlacing | null;
	startPlacing(placing: CadPlacing): void;
	stopPlacing(): void;
	/** Places one copy of the element being placed at a world point, in metres. */
	placeAt(position: { x: number; y: number; z: number }): Promise<void>;
}

/** The element repeated placement puts down. */
export interface CadPlacing {
	profileId: string;
	name: string;
	/** A catalogue part's options and size, which every copy is placed with. */
	with?: PlacedWith;
}

/**
 * One press of an add button's worth of placement; `request` tells two of the same apart.
 *
 * A wizard places a whole field at once, so this carries every object placed rather than one: the
 * CAD screen selects them all and opens Info on the first.
 */
export interface CadPlaced {
	fixtureIds: readonly string[];
	request: number;
}

const NO_TOOLS: CadTools = {
	onAdd: null,
	tool: "select",
	setTool: () => undefined,
	annotations: [],
	save: async () => undefined,
	remove: async () => undefined,
	change: async () => undefined,
	selectedTextId: null,
	selectText: () => undefined,
	textPreview: null,
	setTextPreview: () => undefined,
	error: null,
	clearError: () => undefined,
	placed: null,
	announcePlaced: () => undefined,
	placing: null,
	startPlacing: () => undefined,
	stopPlacing: () => undefined,
	placeAt: async () => undefined,
};

export const CadToolContext = createContext<CadTools>(NO_TOOLS);

export function useCadTools() {
	return useContext(CadToolContext);
}

/** The drawn items of the open show and the tool in hand, for one CAD screen. */
export function CadToolProvider({
	documentKey,
	onAdd,
	children,
}: {
	documentKey: string;
	onAdd: (kind: CadAddKind, profileId?: string, several?: boolean) => void;
	children: ReactNode;
}) {
	const [placed, setPlaced] = useState<CadPlaced | null>(null);
	const [tool, setTool] = useState<CadDrawTool>("select");
	const [annotations, setAnnotations] = useState<CadAnnotation[]>([]);
	const [selectedTextId, selectText] = useState<string | null>(null);
	const [textPreview, setTextPreview] = useState<CadTools["textPreview"]>(null);
	const [error, setError] = useState<string | null>(null);
	const [placing, setPlacing] = useState<CadPlacing | null>(null);

	useEffect(() => {
		let disposed = false;
		let unlisten: (() => void) | undefined;
		annotationSession
			.all()
			.then((loaded) => !disposed && setAnnotations(loaded))
			.catch(() => !disposed && setAnnotations([]));
		annotationSession
			.onDelta((next) => !disposed && setAnnotations(next))
			.then((stop) => {
				if (disposed) stop();
				else unlisten = stop;
			})
			.catch(() => undefined);
		return () => {
			disposed = true;
			unlisten?.();
		};
	}, [documentKey]);

	const value = useMemo<CadTools>(() => {
		const report = (reason: unknown) => setError(String(reason));
		const announcePlaced = (fixtureIds: string | readonly string[]) =>
			setPlaced((current) => ({
				fixtureIds: typeof fixtureIds === "string" ? [fixtureIds] : [...fixtureIds],
				request: (current?.request ?? 0) + 1,
			}));
		return {
			onAdd,
			tool,
			// Picking up another tool puts the element being placed down.
			setTool: (next) => {
				setPlacing(null);
				setTool(next);
			},
			annotations,
			error,
			placed,
			announcePlaced,
			placing,
			startPlacing: (next) => {
				setTool("select");
				setPlacing(next);
			},
			stopPlacing: () => setPlacing(null),
			placeAt: async (position) => {
				if (!placing) return;
				const result = await placeProfile(
					placing.profileId,
					placing.name,
					undefined,
					[{ position, rotation: { x: 0, y: 0, z: 0 } }],
					placing.with,
				);
				if (result.ok) announcePlaced(result.fixtureIds);
				else report(result.reason);
			},
			clearError: () => setError(null),
			save: (annotation) =>
				annotationSession.save(annotation).then(() => undefined, report),
			remove: (id) => annotationSession.remove(id).then(() => undefined, report),
			change: (annotation) =>
				annotationSession.change(annotation).then(() => undefined, report),
			// Text that is no longer drawn cannot stay picked.
			selectedTextId: annotations.some((each) => each.id === selectedTextId)
				? selectedTextId
				: null,
			selectText,
			textPreview,
			setTextPreview,
		};
	}, [onAdd, tool, annotations, error, placed, placing, selectedTextId, textPreview]);

	return <CadToolContext.Provider value={value}>{children}</CadToolContext.Provider>;
}
