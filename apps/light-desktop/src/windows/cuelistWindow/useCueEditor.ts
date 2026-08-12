import {
	type Dispatch,
	type MutableRefObject,
	type SetStateAction,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import type { Cue, CueList, VersionedObject } from "../../api/types";
import {
	type CueListWriteBasis,
	cueListWriteBasis,
	useCueListTopologyWriter,
} from "../../features/playbackTopology/useCueListTopologyWriter";
import { cueDraftIdentity, cueJump } from "./cueFormatting";

interface CueEditorOptions {
	cues: Cue[];
	selectedCueObject: VersionedObject<CueList> | undefined;
	activeCueIndex: number | undefined;
	followActiveCue: boolean;
}

export function useCueEditor({
	cues,
	selectedCueObject,
	activeCueIndex,
	followActiveCue,
}: CueEditorOptions) {
	const saveCueList = useCueListTopologyWriter();
	const cueWriteScope = selectedCueObject?.body.id ?? "";
	const cueWriteQueue = useMemo(
		() => createCueWriteQueue(saveCueList),
		[cueWriteScope, saveCueList],
	);
	const activeCueWriteQueue = useRef(cueWriteQueue);
	activeCueWriteQueue.current = cueWriteQueue;
	const latestCueObject = useRef(selectedCueObject);
	latestCueObject.current = selectedCueObject;
	const cueWriteNeedsRebase = useRef(false);
	const [selectedCue, setSelectedCue] = useState(0);
	const [cueDraft, setCueDraftState] = useState<Cue | null>(null);
	const cueDraftRef = useRef<Cue | null>(null);
	const setCueDraft = useCallback<Dispatch<SetStateAction<Cue | null>>>(
		(value) =>
			setCueDraftState((current) => {
				const next = typeof value === "function" ? value(current) : value;
				cueDraftRef.current = next;
				return next;
			}),
		[],
	);
	const [cueEditError, setCueEditError] = useState("");
	const cueServerSnapshot = useRef<{
		authority: CueWriteQueue;
		identity: string | null;
		serialized: string;
	} | null>(null);
	const cueWriteBase = useRef<{
		basis: CueListWriteBasis;
		body: CueList;
		cueIndex: number;
	} | null>(null);

	useEffect(() => {
		if (!followActiveCue || activeCueIndex == null || !cues[activeCueIndex])
			return;
		setSelectedCue(activeCueIndex);
	}, [activeCueIndex, cues, followActiveCue]);

	useEffect(() => {
		const next = cues[selectedCue] ? { ...cues[selectedCue] } : null;
		const nextSnapshot = {
			authority: cueWriteQueue,
			identity: cueDraftIdentity(next),
			serialized: JSON.stringify(next),
		};
		const previousSnapshot = cueServerSnapshot.current;
		cueServerSnapshot.current = nextSnapshot;
		const currentSerialized = JSON.stringify(cueDraftRef.current);
		const sameCue =
			previousSnapshot?.authority === nextSnapshot.authority &&
			nextSnapshot.identity != null &&
			cueDraftIdentity(cueDraftRef.current) === nextSnapshot.identity &&
			previousSnapshot?.identity === nextSnapshot.identity;
		const locallyEdited =
			sameCue && currentSerialized !== previousSnapshot?.serialized;
		const serverCaughtUp = currentSerialized === nextSnapshot.serialized;
		const rebaseIdentity = cueDraftIdentity(cueDraftRef.current);
		const previousBasis = cueWriteBase.current?.basis;
		const repairedAuthority =
			selectedCueObject &&
			(previousBasis?.expectedObjectId !== selectedCueObject.id ||
				previousBasis.expectedRevision !== selectedCueObject.revision);
		const rebaseIndex =
			cueWriteNeedsRebase.current && repairedAuthority && rebaseIdentity
				? findCue(selectedCueObject.body, rebaseIdentity)
				: -1;
		if (selectedCueObject && rebaseIndex >= 0) {
			cueWriteNeedsRebase.current = false;
			cueWriteBase.current = {
				basis: cueListWriteBasis(selectedCueObject),
				body: selectedCueObject.body,
				cueIndex: rebaseIndex,
			};
			return;
		}
		if (!locallyEdited || serverCaughtUp) {
			cueWriteNeedsRebase.current = false;
			cueDraftRef.current = next;
			setCueDraftState(next);
			cueWriteBase.current = selectedCueObject
				? {
						basis: cueListWriteBasis(selectedCueObject),
						body: selectedCueObject.body,
						cueIndex: selectedCue,
					}
				: null;
		}
	}, [
		selectedCue,
		selectedCueObject?.id,
		selectedCueObject?.revision,
		selectedCueObject?.body,
		cues,
		cueWriteQueue,
	]);

	useEffect(() => setCueEditError(""), [cueWriteQueue]);

	useEffect(() => {
		setSelectedCue((current) =>
			Math.min(current, Math.max(0, cues.length - 1)),
		);
	}, [selectedCueObject?.body.id, cues.length]);

	const saveCue = async (nextCue = cueDraft) => {
		return queueCueWrite(nextCue, {
			queue: cueWriteQueue,
			activeQueue: activeCueWriteQueue,
			latestCueObject,
			cueDraft: cueDraftRef,
			needsRebase: cueWriteNeedsRebase,
			cueWriteBase,
			setCueEditError,
		});
	};

	return {
		selectedCue,
		setSelectedCue,
		cueDraft,
		setCueDraft,
		cueEditError,
		saveCue,
	};
}

interface CueWriteQueue {
	save: ReturnType<typeof useCueListTopologyWriter>;
	tail: Promise<void>;
	generation: number;
	pending: Set<string>;
	completedKey: string | null;
}

interface QueuedCueWrite {
	cueIdentity: string;
	nextCue: Cue;
	cueListId: string;
	generation: number;
	saveKey: string;
	queue: CueWriteQueue;
	activeQueue: MutableRefObject<CueWriteQueue>;
	latestCueObject: MutableRefObject<VersionedObject<CueList> | undefined>;
	cueDraft: MutableRefObject<Cue | null>;
	needsRebase: MutableRefObject<boolean>;
	cueWriteBase: MutableRefObject<{
		basis: CueListWriteBasis;
		body: CueList;
		cueIndex: number;
	} | null>;
	setCueEditError: Dispatch<SetStateAction<string>>;
}

type CueWriteControls = Omit<
	QueuedCueWrite,
	"cueIdentity" | "nextCue" | "cueListId" | "generation" | "saveKey"
>;

async function queueCueWrite(
	nextCue: Cue | null,
	controls: CueWriteControls,
): Promise<boolean> {
	const writeBase = controls.cueWriteBase.current;
	if (!nextCue || !writeBase) return false;
	if (!hasValidTimings(nextCue)) {
		controls.setCueEditError(
			"Cue edit was not saved. Timings must be zero or greater and Jump Count must be a whole number of one or greater.",
		);
		return false;
	}
	const cueIdentity = cueDraftIdentity(nextCue);
	if (!cueIdentity) return false;
	const saveKey = cueWriteKey(writeBase, cueIdentity, nextCue);
	if (controls.queue.completedKey === saveKey) return true;
	if (controls.queue.pending.has(saveKey)) {
		await controls.queue.tail;
		return controls.queue.completedKey === saveKey;
	}
	controls.queue.pending.add(saveKey);
	controls.setCueEditError("");
	const generation = controls.queue.generation;
	const operation = () =>
		commitQueuedCue({
			...controls,
			cueIdentity,
			nextCue,
			cueListId: writeBase.basis.cueListId,
			generation,
			saveKey,
		});
	const result = controls.queue.tail.then(operation, operation);
	controls.queue.tail = result.then(
		() => undefined,
		() => undefined,
	);
	return result;
}

async function commitQueuedCue(write: QueuedCueWrite): Promise<boolean> {
	try {
		if (!isCurrentWrite(write)) return false;
		const current = write.cueWriteBase.current;
		if (!current || current.basis.cueListId !== write.cueListId) return false;
		const cueIndex = findCue(current.body, write.cueIdentity);
		if (cueIndex < 0) return false;
		const saved = await write.queue.save(current.basis, {
			...current.body,
			cues: current.body.cues.map((cue, index) =>
				index === cueIndex ? write.nextCue : cue,
			),
		});
		if (!isCurrentWrite(write)) return false;
		if (saved) return installQueuedWrite(write, saved);
		failQueuedWrite(write);
		return false;
	} catch {
		if (isCurrentWrite(write)) failQueuedWrite(write);
		return false;
	} finally {
		write.queue.pending.delete(write.saveKey);
	}
}

function installQueuedWrite(
	write: QueuedCueWrite,
	saved: VersionedObject<CueList>,
): boolean {
	const current = write.cueWriteBase.current;
	if (!current) return false;
	const latest = write.latestCueObject.current;
	if (
		latest?.body.id === saved.body.id &&
		(latest.id !== saved.id || latest.revision > saved.revision)
	) {
		cancelQueuedWrites(write.queue);
		adoptWriteBase(write, latest);
		write.setCueEditError(
			"Cue edit was saved, but newer Cuelist changes also arrived. Review the current values before saving again.",
		);
		return false;
	}
	const selectedIdentity = cueDraftIdentity(
		current.body.cues[current.cueIndex],
	);
	const selectedIndex = selectedIdentity
		? findCue(saved.body, selectedIdentity)
		: -1;
	write.cueWriteBase.current = {
		basis: cueListWriteBasis(saved),
		body: saved.body,
		cueIndex: selectedIndex >= 0 ? selectedIndex : current.cueIndex,
	};
	write.needsRebase.current = false;
	write.queue.completedKey = cueWriteKey(
		write.cueWriteBase.current,
		write.cueIdentity,
		write.nextCue,
	);
	return true;
}

function failQueuedWrite(write: QueuedCueWrite) {
	cancelQueuedWrites(write.queue);
	const latest = write.latestCueObject.current;
	const basis = write.cueWriteBase.current?.basis;
	if (
		latest?.body.id === write.cueListId &&
		(basis?.expectedObjectId !== latest.id ||
			basis.expectedRevision !== latest.revision)
	) {
		adoptWriteBase(write, latest);
		write.needsRebase.current = false;
	} else write.needsRebase.current = true;
	write.setCueEditError(
		"Cue edit was not saved. Review the draft and try again after the revision conflict.",
	);
}

function adoptWriteBase(
	write: QueuedCueWrite,
	object: VersionedObject<CueList>,
) {
	const selectedIdentity =
		cueDraftIdentity(write.cueDraft.current) ?? write.cueIdentity;
	const selectedIndex = findCue(object.body, selectedIdentity);
	write.cueWriteBase.current = {
		basis: cueListWriteBasis(object),
		body: object.body,
		cueIndex: selectedIndex >= 0 ? selectedIndex : 0,
	};
	write.needsRebase.current = false;
}

function isCurrentWrite(write: QueuedCueWrite) {
	return (
		write.activeQueue.current === write.queue &&
		write.queue.generation === write.generation
	);
}

function cancelQueuedWrites(queue: CueWriteQueue) {
	queue.generation += 1;
	queue.pending.clear();
	queue.completedKey = null;
}

function createCueWriteQueue(
	save: ReturnType<typeof useCueListTopologyWriter>,
): CueWriteQueue {
	return {
		save,
		tail: Promise.resolve(),
		generation: 0,
		pending: new Set(),
		completedKey: null,
	};
}

function findCue(cueList: CueList, identity: string) {
	return cueList.cues.findIndex((cue) => cueDraftIdentity(cue) === identity);
}

function hasValidTimings(cue: Cue) {
	const jump = cueJump(cue);
	if (jump && (!Number.isSafeInteger(jump.count) || jump.count < 1)) return false;
	const triggerDelay =
		cue.trigger.type === "manual"
			? 0
			: typeof cue.trigger.delay_millis === "number"
				? cue.trigger.delay_millis
				: Number.NaN;
	return [
		cue.fade_millis,
		cue.delay_millis,
		cue.out_fade_millis ?? 0,
		cue.out_delay_millis ?? 0,
		triggerDelay,
	].every((value) => Number.isSafeInteger(value) && value >= 0);
}

function cueWriteKey(
	writeBase: NonNullable<QueuedCueWrite["cueWriteBase"]["current"]>,
	cueIdentity: string,
	nextCue: Cue,
) {
	return [
		writeBase.basis.expectedObjectId,
		writeBase.basis.expectedRevision,
		cueIdentity,
		JSON.stringify(nextCue),
	].join(":");
}
