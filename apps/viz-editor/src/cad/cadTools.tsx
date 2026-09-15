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

/**
 * What the add buttons place: a truss, a stage element, a curtain, a primitive shape (box, cylinder
 * or ball), or any other Venue object.
 */
export type CadAddKind = "truss" | "stage" | "curtain" | "primitive" | "venue";

/** What a press on a viewport does. */
export type CadDrawTool = "select" | "polyline" | "box" | "text" | "measure" | "erase";

export interface CadTools {
	/** Opens the shared fixture library for a kind of object; null hides the toolbar. */
	onAdd: ((kind: CadAddKind) => void) | null;
	tool: CadDrawTool;
	setTool(tool: CadDrawTool): void;
	annotations: readonly CadAnnotation[];
	save(annotation: CadAnnotation): Promise<void>;
	remove(id: string): Promise<void>;
	/** Why the show refused the last drawn item, until the operator dismisses it. */
	error: string | null;
	clearError(): void;
	/** The object an add button placed last, so the CAD screen can select it and open Info. */
	placed: CadPlaced | null;
	/** Tells the CAD screen an add flow has just placed this object. */
	announcePlaced(fixtureId: string): void;
}

/** One placement from an add button; `request` tells two placements of the same object apart. */
export interface CadPlaced {
	fixtureId: string;
	request: number;
}

const NO_TOOLS: CadTools = {
	onAdd: null,
	tool: "select",
	setTool: () => undefined,
	annotations: [],
	save: async () => undefined,
	remove: async () => undefined,
	error: null,
	clearError: () => undefined,
	placed: null,
	announcePlaced: () => undefined,
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
	onAdd: (kind: CadAddKind) => void;
	children: ReactNode;
}) {
	const [placed, setPlaced] = useState<CadPlaced | null>(null);
	const [tool, setTool] = useState<CadDrawTool>("select");
	const [annotations, setAnnotations] = useState<CadAnnotation[]>([]);
	const [error, setError] = useState<string | null>(null);

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
		return {
			onAdd,
			tool,
			setTool,
			annotations,
			error,
			placed,
			announcePlaced: (fixtureId) =>
				setPlaced((current) => ({ fixtureId, request: (current?.request ?? 0) + 1 })),
			clearError: () => setError(null),
			save: (annotation) =>
				annotationSession.save(annotation).then(() => undefined, report),
			remove: (id) => annotationSession.remove(id).then(() => undefined, report),
		};
	}, [onAdd, tool, annotations, error, placed]);

	return <CadToolContext.Provider value={value}>{children}</CadToolContext.Provider>;
}
