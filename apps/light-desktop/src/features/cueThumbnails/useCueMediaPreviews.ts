import {
	type Dispatch,
	type SetStateAction,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import type { CueMediaPreviewEntry } from "../../api/client/cueMediaPreviews";
import { useMediaServers } from "../mediaServers/MediaServersContext";
import { useCueThumbnailActions } from "./CueThumbnailActions";

type CueThumbnailActionsValue = ReturnType<typeof useCueThumbnailActions>;

/** Pixel box a media preview is fitted into; the server keeps the output's aspect ratio. */
export const MEDIA_PREVIEW_SIZE = { width: 320, height: 180 } as const;
/** How often a picture the server is still loading is asked for again, and how many times. */
const LOADING_RETRY_MILLIS = 1_500;
const LOADING_RETRIES = 6;
const PARALLEL_FETCHES = 3;

/**
 * What a Media Server Cue preview shows right now.
 *
 * Every state names the entry it belongs to, so a surface can say which server, output, and
 * layer it is waiting for. Only `ready` and `empty` carry a picture, and that picture is always
 * the one fetched for this exact entry.
 */
export type CueMediaPreview =
	| { state: "loading"; entry: CueMediaPreviewEntry }
	| { state: "ready"; entry: CueMediaPreviewEntry; src: string }
	| { state: "empty"; entry: CueMediaPreviewEntry; src: string }
	| {
			state: "offline" | "missing";
			entry: CueMediaPreviewEntry;
			error: string;
	  };

export interface CueMediaPreviews {
	/** False until the desk has said which Cues are media Cues. */
	ready: boolean;
	previews: ReadonlyMap<string, CueMediaPreview>;
	/** Every wanted Cue a Media Server pictures, known before any picture arrives. */
	mediaCueIds: ReadonlySet<string>;
	/** Asks the Media Server again for pictures that failed. */
	retry(): void;
}

const EMPTY: ReadonlyMap<string, CueMediaPreview> = new Map();
const NO_CUES: ReadonlySet<string> = new Set();

function identity(entry: CueMediaPreviewEntry) {
	return `${entry.cueId}\0${entry.previewKey}`;
}

function releaseExcept(
	urls: Map<string, { src: string }>,
	keep: ReadonlySet<string>,
) {
	for (const [key, picture] of urls) {
		if (keep.has(key)) continue;
		URL.revokeObjectURL(picture.src);
		urls.delete(key);
	}
}

/** Keeps the previous index when nothing changed, so a re-read never restarts the pictures. */
function sameIndex(
	current: CueMediaPreviewEntry[] | null,
	next: CueMediaPreviewEntry[],
): CueMediaPreviewEntry[] {
	if (
		current &&
		current.length === next.length &&
		current.every((entry, index) => {
			const other = next[index];
			return (
				identity(entry) === identity(other) &&
				entry.scope === other.scope &&
				entry.layer === other.layer &&
				entry.serverFixtureId === other.serverFixtureId
			);
		})
	)
		return current;
	return next;
}

type PictureCache = Map<string, { src: string; empty: boolean }>;

/**
 * Fetches the pictures `relevant` names that are not cached yet, a few at a time, and reports
 * each outcome. Returns the cleanup that stops the pass.
 */
function loadPictures(
	actions: CueThumbnailActionsValue | null,
	relevant: CueMediaPreviewEntry[],
	urls: PictureCache,
	setPictures: Dispatch<SetStateAction<ReadonlyMap<string, CueMediaPreview>>>,
): (() => void) | undefined {
	if (!actions || !relevant.length) {
		releaseExcept(urls, new Set());
		setPictures(EMPTY);
		return undefined;
	}
	let cancelled = false;
	const timers: ReturnType<typeof setTimeout>[] = [];
	const keep = new Set(relevant.map(identity));
	// A picture of a state the Cue no longer has is released before anything else happens, so
	// it can never be shown again.
	releaseExcept(urls, keep);
	setPictures((current) => {
		const next = new Map<string, CueMediaPreview>();
		for (const entry of relevant) {
			const known = current.get(entry.cueId);
			next.set(
				entry.cueId,
				known &&
					identity(known.entry) === identity(entry) &&
					(known.state === "ready" || known.state === "empty")
					? known
					: { state: "loading", entry },
			);
		}
		return next;
	});
	const settle = (entry: CueMediaPreview) => {
		if (cancelled) return;
		setPictures((current) => {
			const next = new Map(current);
			next.set(entry.entry.cueId, entry);
			return next;
		});
	};
	const fetchOne = async (entry: CueMediaPreviewEntry, tries = 0) => {
		const key = identity(entry);
		const cached = urls.get(key);
		if (cached) return;
		try {
			const result = await actions.mediaImage(
				entry.cueId,
				entry.previewKey,
				MEDIA_PREVIEW_SIZE,
			);
			if (cancelled) return;
			if (result.kind === "picture") {
				const src = URL.createObjectURL(result.blob);
				urls.set(key, { src, empty: result.empty });
				settle({ state: result.empty ? "empty" : "ready", entry, src });
				return;
			}
			if (result.state === "loading") {
				if (tries + 1 >= LOADING_RETRIES) {
					settle({
						state: "offline",
						entry,
						error: "The Media Server did not finish loading this media.",
					});
					return;
				}
				settle({ state: "loading", entry });
				timers.push(
					setTimeout(() => void fetchOne(entry, tries + 1), LOADING_RETRY_MILLIS),
				);
				return;
			}
			settle({ state: result.state, entry, error: result.error });
		} catch (error) {
			settle({
				state: "offline",
				entry,
				error: error instanceof Error ? error.message : String(error),
			});
		}
	};
	const queue = relevant.filter((entry) => !urls.has(identity(entry)));
	for (const entry of relevant) {
		const picture = urls.get(identity(entry));
		if (!picture) continue;
		settle({
			state: picture.empty ? "empty" : "ready",
			entry,
			src: picture.src,
		});
	}
	const workers = Array.from({ length: PARALLEL_FETCHES }, async () => {
		for (let next = queue.shift(); next; next = queue.shift()) {
			if (cancelled) return;
			await fetchOne(next);
		}
	});
	void Promise.all(workers);
	return () => {
		cancelled = true;
		for (const timer of timers) clearTimeout(timer);
	};
}

/**
 * Media Server pictures for the given Cues.
 *
 * The index is re-read whenever `revision` changes (the caller passes its Cue content) and when a
 * patched Media Server goes online or offline, which the desk pushes. A picture is fetched once
 * per preview key; a changed Cue has a new key and therefore never shows the previous picture.
 */
export function useCueMediaPreviews(
	cueIds: readonly string[],
	active: boolean,
	revision: unknown,
): CueMediaPreviews {
	const actions = useCueThumbnailActions();
	const media = useMediaServers();
	const reachability = useMemo(
		() =>
			(media?.mediaServers ?? [])
				.map((server) => `${server.fixture_id}:${server.status?.online}`)
				.sort()
				.join(","),
		[media?.mediaServers],
	);
	const wanted = useMemo(() => new Set(cueIds), [cueIds]);
	const wantedKey = useMemo(() => [...wanted].sort().join(","), [wanted]);
	const [entries, setEntries] = useState<CueMediaPreviewEntry[] | null>(null);
	const [pictures, setPictures] = useState<ReadonlyMap<string, CueMediaPreview>>(EMPTY);
	const [attempt, setAttempt] = useState(0);
	const urls = useRef<PictureCache>(new Map());

	// biome-ignore lint/correctness/useExhaustiveDependencies: revision, reachability, and attempt are deliberate re-read triggers.
	useEffect(() => {
		if (!active || !actions?.available) {
			// Without a desk to ask, no Cue is a media Cue and Stage previews proceed at once.
			setEntries((current) => (active ? sameIndex(current, []) : null));
			return;
		}
		let cancelled = false;
		actions
			.mediaIndex()
			.then((index) => {
				if (!cancelled) setEntries((current) => sameIndex(current, index));
			})
			.catch(() => {
				// A desk that cannot list media previews keeps every Cue on its Stage preview.
				if (!cancelled) setEntries((current) => sameIndex(current, []));
			});
		return () => {
			cancelled = true;
		};
	}, [active, actions, revision, reachability, attempt]);

	const relevant = useMemo(
		() => (entries ?? []).filter((entry) => wanted.has(entry.cueId)),
		// wantedKey keeps a new-but-equal id list from refetching.
		// biome-ignore lint/correctness/useExhaustiveDependencies: wantedKey stands in for wanted.
		[entries, wantedKey],
	);

	// biome-ignore lint/correctness/useExhaustiveDependencies: attempt and reachability ask failed pictures again.
	useEffect(
		() => loadPictures(actions, relevant, urls.current, setPictures),
		[actions, relevant, attempt, reachability],
	);

	useEffect(
		() => () => {
			for (const picture of urls.current.values())
				URL.revokeObjectURL(picture.src);
			urls.current.clear();
		},
		[],
	);

	const relevantKey = relevant.map((entry) => entry.cueId).join(",");
	// biome-ignore lint/correctness/useExhaustiveDependencies: relevantKey stands in for relevant.
	const mediaCueIds = useMemo(
		() => (relevant.length ? new Set(relevant.map((entry) => entry.cueId)) : NO_CUES),
		[relevantKey],
	);
	const retry = useCallback(() => setAttempt((value) => value + 1), []);
	return {
		ready: entries !== null,
		previews: active ? pictures : EMPTY,
		mediaCueIds: active ? mediaCueIds : NO_CUES,
		retry,
	};
}
