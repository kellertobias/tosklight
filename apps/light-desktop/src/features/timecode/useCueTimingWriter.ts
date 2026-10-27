import { mergeCueTimingFields } from "./mergeCueTimingFields";
import { useCallback, useEffect, useRef, useState } from "react";
import type { CueList } from "../../api/types";
import type { SaveCueListTopology } from "../playbackTopology/useCueListTopologyWriter";
import type { TimecodeCueListOption } from "./timecodeEditorShared";

/** Where the desk left a Cuelist, so a run of edits keeps writing against the newest revision. */
interface CueListBasis {
	objectId: string;
	revision: number;
}

export interface CueTimingWriter {
	/** The Cuelists to draw, carrying edits the desk has not answered yet. */
	cueLists: TimecodeCueListOption[];
	saving: boolean;
	error: string | null;
	/** Records the edit for drawing at once, then writes it through Cuelist authority. */
	save(cueListId: string, body: CueList): Promise<CueList>;
	current(cueListId: string): CueList | undefined;
	restore(cueListId: string, expected: CueList, target: CueList): Promise<void>;
}

function withBody(
	option: TimecodeCueListOption,
	body: CueList,
): TimecodeCueListOption {
	return { ...option, cues: body.cues, body };
}

/**
 * Turning an encoder produces a stream of small timing edits, and the lane has to follow the hand
 * rather than the network.
 *
 * Each edit is drawn immediately and only then written. A write that arrives while another is in
 * flight replaces the one waiting instead of being refused, so a fast turn lands as one save of
 * where the encoder came to rest rather than a queue of intermediate positions.
 */
export function useCueTimingWriter(
	cueLists: TimecodeCueListOption[],
	saveCueList: SaveCueListTopology | null,
): CueTimingWriter {
	/** Edits the desk has not answered, dropped once its own answer is held instead. */
	const [live, setLive] = useState<Map<string, CueList>>(new Map());
	/** What the desk answered, dropped once the authoritative Cuelists catch up. */
	const [settled, setSettled] = useState<Map<string, TimecodeCueListOption>>(
		new Map(),
	);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const waiting = useRef(
		new Map<
			string,
			{
				body: CueList;
				waiters: Array<{
					resolve(body: CueList): void;
					reject(cause: unknown): void;
				}>;
			}
		>(),
	);
	const writing = useRef(new Set<string>());
	const basis = useRef(new Map<string, CueListBasis>());
	const sources = useRef(cueLists);
	sources.current = cueLists;

	useEffect(() => {
		setSettled((current) => {
			let changed = false;
			const next = new Map(current);
			for (const cueList of cueLists) {
				const local = next.get(cueList.id);
				if (local && (cueList.revision ?? 0) >= (local.revision ?? 0)) {
					next.delete(cueList.id);
					basis.current.delete(cueList.id);
					changed = true;
				}
			}
			return changed ? next : current;
		});
	}, [cueLists]);

	const writeBasis = useCallback((cueListId: string): CueListBasis => {
		const source = sources.current.find((item) => item.id === cueListId);
		const known = basis.current.get(cueListId);
		if (known && known.revision >= (source?.revision ?? 0)) return known;
		if (!source?.objectId || source.revision === undefined)
			throw new Error("The authoritative Cue List writer is unavailable.");
		return { objectId: source.objectId, revision: source.revision };
	}, []);

	const drain = useCallback(
		async (cueListId: string) => {
			if (writing.current.has(cueListId)) return;
			writing.current.add(cueListId);
			setSaving(true);
			let active:
				| NonNullable<ReturnType<typeof waiting.current.get>>
				| undefined;
			try {
				for (;;) {
					active = waiting.current.get(cueListId);
					if (!active) break;
					const { body } = active;
					waiting.current.delete(cueListId);
					if (!saveCueList)
						throw new Error("The authoritative Cue List writer is unavailable.");
					const { objectId, revision } = writeBasis(cueListId);
					const saved = await saveCueList(
						{ cueListId, expectedRevision: revision, expectedObjectId: objectId },
						body,
					);
					if (!saved)
						throw new Error("The desk did not return the saved Cue List.");
					basis.current.set(cueListId, {
						objectId: saved.id,
						revision: saved.revision,
					});
					setSettled((current) =>
						new Map(current).set(cueListId, {
							id: saved.body.id,
							name: saved.body.name,
							cues: saved.body.cues,
							objectId: saved.id,
							revision: saved.revision,
							body: saved.body,
						}),
					);
					for (const waiter of active.waiters) waiter.resolve(saved.body);
					active = undefined;
				}
				setError(null);
			} catch (cause) {
				setError(
					cause instanceof Error
						? cause.message
						: "The Cue timing did not save.",
				);
				for (const waiter of active?.waiters ?? []) waiter.reject(cause);
				for (const waiter of waiting.current.get(cueListId)?.waiters ?? [])
					waiter.reject(cause);
				waiting.current.delete(cueListId);
			} finally {
				writing.current.delete(cueListId);
				setSaving(writing.current.size > 0);
				// The desk's own answer is held now, so the drawn-ahead edit has nothing left to say.
				if (!waiting.current.has(cueListId))
					setLive((current) => {
						if (!current.has(cueListId)) return current;
						const next = new Map(current);
						next.delete(cueListId);
						return next;
					});
			}
		},
		[saveCueList, writeBasis],
	);

	const save = useCallback(
		(cueListId: string, body: CueList): Promise<CueList> => {
			setLive((current) => new Map(current).set(cueListId, body));
			const result = new Promise<CueList>((resolve, reject) => {
				const previous = waiting.current.get(cueListId);
				waiting.current.set(cueListId, {
					body,
					waiters: [...(previous?.waiters ?? []), { resolve, reject }],
				});
			});
			void drain(cueListId);
			return result;
		},
		[drain],
	);
	const drawn = cueLists.map((cueList) => {
		const option = settled.get(cueList.id) ?? cueList;
		const body = live.get(cueList.id);
		return body ? withBody(option, body) : option;
	});
	return {
		cueLists: drawn,
		saving,
		error,
		save,
		...useCueTimingHistory(drawn, save),
	};
}

function useCueTimingHistory(
	drawn: TimecodeCueListOption[],
	save: CueTimingWriter["save"],
) {
	const latest = useRef(drawn);
	latest.current = drawn;
	const current = useCallback(
		(cueListId: string) =>
			latest.current.find((item) => item.id === cueListId)?.body,
		[],
	);
	const restore = useCallback(
		async (cueListId: string, expected: CueList, target: CueList) => {
			const body = current(cueListId);
			if (!body) throw new Error("The authoritative Cue List is unavailable.");
			const next = mergeCueTimingFields(body, expected, target);
			if (next !== body) await save(cueListId, next);
		},
		[current, save],
	);
	return { current, restore };
}
