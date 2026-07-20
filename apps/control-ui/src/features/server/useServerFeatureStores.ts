import { useRef } from "react";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { ProgrammerLifecycleStore } from "../programmerLifecycle/store";
import { ProgrammerPreloadPlaybackQueueStore } from "../programmerPreloadPlaybackQueue/store";
import { ProgrammerPreloadValuesStore } from "../programmerPreloadValues/store";
import { ProgrammerValuesStore } from "../programmerValues/store";
import { ProgrammingInteractionStore } from "../programmingInteraction/store";

/** Stable external stores kept outside the broad React server-state update path. */
export function useServerFeatureStores() {
	return {
		playbackRuntimeStore: useRef(new PlaybackRuntimeStore()).current,
		programmingInteractionStore: useRef(new ProgrammingInteractionStore())
			.current,
		programmerCaptureModeStore: useRef(new ProgrammerCaptureModeStore())
			.current,
		programmerLifecycleStore: useRef(new ProgrammerLifecycleStore()).current,
		programmerValuesStore: useRef(new ProgrammerValuesStore()).current,
		programmerPreloadValuesStore: useRef(new ProgrammerPreloadValuesStore())
			.current,
		programmerPreloadPlaybackQueueStore: useRef(
			new ProgrammerPreloadPlaybackQueueStore(),
		).current,
	};
}
