import { useRef } from "react";
import { ConfigurationStore } from "../configuration/store";
import { OutputRuntimeStore } from "../outputRuntime/store";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { ProgrammerLifecycleStore } from "../programmerLifecycle/store";
import { ProgrammerPreloadLifecycleStore } from "../programmerPreloadLifecycle/store";
import { ProgrammerPreloadPlaybackQueueStore } from "../programmerPreloadPlaybackQueue/store";
import { ProgrammerPreloadValuesStore } from "../programmerPreloadValues/store";
import { ProgrammerPriorityStore } from "../programmerPriority/store";
import { ProgrammerValuesStore } from "../programmerValues/store";
import { ProgrammingInteractionStore } from "../programmingInteraction/store";
import { SpeedGroupRuntimeStore } from "../speedGroupRuntime/store";
import { StageLayoutStore } from "../stageLayout/store";

/** Stable external stores kept outside the broad React server-state update path. */
export function useServerFeatureStores() {
	const outputRuntimeStore = useRef<OutputRuntimeStore | null>(null);
	outputRuntimeStore.current ??= new OutputRuntimeStore();
	const speedGroupRuntimeStore = useRef<SpeedGroupRuntimeStore | null>(null);
	speedGroupRuntimeStore.current ??= new SpeedGroupRuntimeStore();
	return {
		configurationStore: useRef(new ConfigurationStore()).current,
		stageLayoutStore: useRef(new StageLayoutStore()).current,
		outputRuntimeStore: outputRuntimeStore.current,
		speedGroupRuntimeStore: speedGroupRuntimeStore.current,
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
