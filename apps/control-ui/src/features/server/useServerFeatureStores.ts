import { useRef } from "react";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import { OutputRuntimeStore } from "../outputRuntime/store";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { ProgrammerLifecycleStore } from "../programmerLifecycle/store";
import { ProgrammerPreloadPlaybackQueueStore } from "../programmerPreloadPlaybackQueue/store";
import { ProgrammerPreloadLifecycleStore } from "../programmerPreloadLifecycle/store";
import { ProgrammerPreloadValuesStore } from "../programmerPreloadValues/store";
import { ProgrammerPriorityStore } from "../programmerPriority/store";
import { ProgrammerValuesStore } from "../programmerValues/store";
import { ProgrammingInteractionStore } from "../programmingInteraction/store";

/** Stable external stores kept outside the broad React server-state update path. */
export function useServerFeatureStores() {
	const outputRuntimeStore = useRef<OutputRuntimeStore | null>(null);
	outputRuntimeStore.current ??= new OutputRuntimeStore();
	return {
		outputRuntimeStore: outputRuntimeStore.current,
		playbackRuntimeStore: useRef(new PlaybackRuntimeStore()).current,
		programmingInteractionStore: useRef(new ProgrammingInteractionStore())
			.current,
		programmerCaptureModeStore: useRef(new ProgrammerCaptureModeStore())
			.current,
		programmerLifecycleStore: useRef(new ProgrammerLifecycleStore()).current,
		programmerPriorityStore: useRef(new ProgrammerPriorityStore()).current,
		programmerValuesStore: useRef(new ProgrammerValuesStore()).current,
		programmerPreloadValuesStore: useRef(new ProgrammerPreloadValuesStore())
			.current,
		programmerPreloadPlaybackQueueStore: useRef(
			new ProgrammerPreloadPlaybackQueueStore(),
		).current,
		programmerPreloadLifecycleStore: useRef(
			new ProgrammerPreloadLifecycleStore(),
		).current,
	};
}
