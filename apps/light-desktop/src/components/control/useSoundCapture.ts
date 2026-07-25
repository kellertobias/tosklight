import {
	type Dispatch,
	type RefObject,
	type SetStateAction,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import { useSoundToLightActions } from "../../features/soundToLight/SoundToLightContext";
import type {
	SoundObservation,
	SoundToLightConfig,
	SpeedGroupId,
	SpeedGroupSoundState,
} from "../../api/types";
import {
	inactiveCaptureStatus,
	type MicrophonePermission,
	type SoundCaptureStatus,
	SoundToLightAudioAnalyzer,
} from "./soundToLightAnalyzer";
import {
	type SoundGroupMap,
	soundToLightErrorMessage,
	speedGroupIds,
} from "./soundToLightModel";

interface SoundCaptureOptions {
	enabled: boolean;
	states: SoundGroupMap<SpeedGroupSoundState>;
	previews: SoundGroupMap<SoundToLightConfig>;
	deviceIds: SoundGroupMap<string>;
	mounted: RefObject<boolean>;
	acceptState: (state: SpeedGroupSoundState) => SpeedGroupSoundState;
	setError: (error: string | null) => void;
	setPermission: (permission: MicrophonePermission) => void;
	refreshInputs: () => Promise<void>;
}

type AnalyzerMap = Map<
	SpeedGroupId,
	{ deviceId: string; analyzer: SoundToLightAudioAnalyzer }
>;

export function shouldPublishSoundObservation(
	state: SpeedGroupSoundState | undefined,
) {
	return state?.configuration.enabled === true;
}

function useSoundObservationPublisher(
	options: Pick<SoundCaptureOptions, "acceptState" | "mounted" | "setError">,
) {
	const { acceptState, mounted, setError } = options;
	const soundActions = useSoundToLightActions();
	const serverRef = useRef(soundActions);
	serverRef.current = soundActions;
	const latestObservations = useRef<SoundGroupMap<SoundObservation>>({});
	const posting = useRef(new Set<SpeedGroupId>());
	const retryAfter = useRef<SoundGroupMap<number>>({});
	return useCallback(
		(group: SpeedGroupId, observation: SoundObservation) => {
			latestObservations.current[group] = observation;
			if (
				posting.current.has(group) ||
				Date.now() < (retryAfter.current[group] ?? 0)
			) {
				return;
			}
			posting.current.add(group);
			void (async () => {
				try {
					while (latestObservations.current[group] && mounted.current) {
						const next = latestObservations.current[group];
						delete latestObservations.current[group];
						const sound = serverRef.current;
						if (!sound) break;
						acceptState(await sound.observeSpeedGroup(group, next));
					}
					setError(null);
				} catch (reason) {
					delete latestObservations.current[group];
					retryAfter.current[group] = Date.now() + 1_000;
					if (mounted.current) {
						setError(
							`Speed Group ${group} audio feedback failed: ${soundToLightErrorMessage(reason)}`,
						);
					}
				} finally {
					posting.current.delete(group);
				}
			})();
		},
		[acceptState, mounted, setError],
	);
}

function stopAnalyzers(analyzers: AnalyzerMap) {
	analyzers.forEach(({ analyzer }) => {
		analyzer.stop();
	});
	analyzers.clear();
}

function useAnalyzerCaptures(
	options: Omit<SoundCaptureOptions, "acceptState" | "setError">,
	postObservation: (group: SpeedGroupId, value: SoundObservation) => void,
) {
	const {
		deviceIds,
		enabled,
		mounted,
		previews,
		refreshInputs,
		setPermission,
		states,
	} = options;
	const statesRef = useRef(states);
	statesRef.current = states;
	const [captures, setCaptures] = useState<SoundGroupMap<SoundCaptureStatus>>(
		{},
	);
	const analyzers = useRef<AnalyzerMap>(new Map());
	useEffect(() => {
		if (!enabled) {
			stopAnalyzers(analyzers.current);
			setCaptures((current) =>
				Object.keys(current).length === 0 ? current : {},
			);
			return;
		}
		for (const group of speedGroupIds) {
			reconcileGroupCapture(group, {
				saved: states[group]?.configuration,
				preview: previews[group],
				deviceId: deviceIds[group] ?? "",
				analyzers: analyzers.current,
				setCaptures,
				observe: (observation) => {
					if (shouldPublishSoundObservation(statesRef.current[group])) {
						postObservation(group, observation);
					}
				},
				status: (status) => {
					if (!mounted.current) return;
					setCaptures((current) => ({ ...current, [group]: status }));
					if (status.phase === "capturing") {
						setPermission("granted");
						if (!status.observation) void refreshInputs();
					}
					if (status.phase === "permission_denied") setPermission("denied");
				},
			});
		}
	}, [
		deviceIds,
		enabled,
		mounted,
		postObservation,
		previews,
		refreshInputs,
		setPermission,
		states,
	]);
	useEffect(() => () => stopAnalyzers(analyzers.current), []);
	return captures;
}

export function useSoundCapture(options: SoundCaptureOptions) {
	const postObservation = useSoundObservationPublisher(options);
	return useAnalyzerCaptures(options, postObservation);
}

interface GroupCaptureContext {
	saved: SoundToLightConfig | undefined;
	preview: SoundToLightConfig | undefined;
	deviceId: string;
	analyzers: AnalyzerMap;
	setCaptures: Dispatch<SetStateAction<SoundGroupMap<SoundCaptureStatus>>>;
	observe: (observation: SoundObservation) => void;
	status: (status: SoundCaptureStatus) => void;
}

/// Starts, retunes, or stops one Speed Group's audio analyzer to match its configuration.
function reconcileGroupCapture(
	group: SpeedGroupId,
	context: GroupCaptureContext,
) {
	const { saved, preview, deviceId, analyzers, setCaptures } = context;
	const configuration = preview ?? saved;
	const shouldCapture = Boolean(
		configuration && deviceId && (saved?.enabled || preview),
	);
	const running = analyzers.get(group);
	if (!shouldCapture || !configuration) {
		if (running) {
			running.analyzer.stop();
			analyzers.delete(group);
		}
		setCaptures((current) => ({
			...current,
			[group]: deviceId
				? inactiveCaptureStatus
				: {
						...inactiveCaptureStatus,
						message: saved?.enabled
							? "Sound-to-Light is enabled, but this browser has no desk-local input assignment."
							: inactiveCaptureStatus.message,
					},
		}));
		return;
	}
	if (running?.deviceId === deviceId) {
		running.analyzer.updateConfiguration(configuration);
		return;
	}
	running?.analyzer.stop();
	const analyzer = new SoundToLightAudioAnalyzer(
		configuration,
		context.observe,
		context.status,
	);
	analyzers.set(group, { deviceId, analyzer });
	void analyzer.start(deviceId);
}
