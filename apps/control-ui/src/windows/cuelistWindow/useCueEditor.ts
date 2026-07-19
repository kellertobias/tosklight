import { useEffect, useRef, useState } from "react";
import { useServer } from "../../api/ServerContext";
import type { Cue, CueList, VersionedObject } from "../../api/types";
import { cueDraftIdentity } from "./cueFormatting";

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
	const server = useServer();
	const [selectedCue, setSelectedCue] = useState(0);
	const [cueDraft, setCueDraft] = useState<Cue | null>(null);
	const [cueEditError, setCueEditError] = useState("");
	const cueServerSnapshot = useRef<{
		identity: string | null;
		serialized: string;
	} | null>(null);
	const cueSavePending = useRef("");

	useEffect(() => {
		if (!followActiveCue || activeCueIndex == null || !cues[activeCueIndex])
			return;
		setSelectedCue(activeCueIndex);
	}, [activeCueIndex, cues, followActiveCue]);

	useEffect(() => {
		const next = cues[selectedCue] ? { ...cues[selectedCue] } : null;
		const nextSnapshot = {
			identity: cueDraftIdentity(next),
			serialized: JSON.stringify(next),
		};
		const previousSnapshot = cueServerSnapshot.current;
		cueServerSnapshot.current = nextSnapshot;
		setCueDraft((current) => {
			const currentSerialized = JSON.stringify(current);
			const sameCue =
				nextSnapshot.identity != null &&
				cueDraftIdentity(current) === nextSnapshot.identity &&
				previousSnapshot?.identity === nextSnapshot.identity;
			const locallyEdited =
				sameCue && currentSerialized !== previousSnapshot.serialized;
			const serverCaughtUp = currentSerialized === nextSnapshot.serialized;
			// Event refreshes can return the last saved Cue while an operator is typing.
			if (locallyEdited && !serverCaughtUp) return current;
			return next;
		});
	}, [selectedCue, cues]);

	useEffect(() => {
		setSelectedCue((current) =>
			Math.min(current, Math.max(0, cues.length - 1)),
		);
	}, [selectedCueObject?.body.id, cues.length]);

	const saveCue = async (nextCue = cueDraft) => {
		if (!nextCue || !selectedCueObject) return;
		const triggerDelay =
			nextCue.trigger.type === "manual"
				? 0
				: typeof nextCue.trigger.delay_millis === "number"
					? nextCue.trigger.delay_millis
					: Number.NaN;
		const timings = [nextCue.fade_millis, nextCue.delay_millis, triggerDelay];
		if (timings.some((value) => !Number.isSafeInteger(value) || value < 0)) {
			cueSavePending.current = "";
			setCueEditError(
				"Cue edit was not saved. Fade, Delay, and Trigger time must be zero or greater.",
			);
			return;
		}
		const saveKey = `${selectedCueObject.id}:${selectedCue}:${JSON.stringify(nextCue)}`;
		if (cueSavePending.current === saveKey) return;
		cueSavePending.current = saveKey;
		setCueEditError("");
		const saved = await server.saveCueList(
			{
				...selectedCueObject.body,
				cues: selectedCueObject.body.cues.map((cue, index) =>
					index === selectedCue ? nextCue : cue,
				),
			},
			selectedCueObject.revision,
		);
		if (!saved) {
			cueSavePending.current = "";
			setCueEditError(
				"Cue edit was not saved. Check the value or refresh after a revision conflict.",
			);
		}
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
