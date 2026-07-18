import {
	type RefObject,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import { useServer } from "../../api/ServerContext";
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
	states: SoundGroupMap<SpeedGroupSoundState>;
	previews: SoundGroupMap<SoundToLightConfig>;
	deviceIds: SoundGroupMap<string>;
	mounted: RefObject<boolean>;
	acceptState: (state: SpeedGroupSoundState) => SpeedGroupSoundState;
	setError: (error: string | null) => void;
	setPermission: (permission: MicrophonePermission) => void;
	refreshInputs: () => Promise<void>;
}

export function shouldPublishSoundObservation(
	state: SpeedGroupSoundState | undefined,
) {
	return state?.configuration.enabled === true;
}

export function useSoundCapture({
	states,
	previews,
	deviceIds,
	mounted,
	acceptState,
	setError,
	setPermission,
	refreshInputs,
}: SoundCaptureOptions) {
	const server = useServer();
	const serverRef = useRef(server);
	serverRef.current = server;
	const statesRef = useRef(states);
	statesRef.current = states;
	const [captures, setCaptures] = useState<SoundGroupMap<SoundCaptureStatus>>(
		{},
	);
	const analyzers = useRef(
		new Map<
			SpeedGroupId,
			{ deviceId: string; analyzer: SoundToLightAudioAnalyzer }
		>(),
	);
	const latestObservations = useRef<SoundGroupMap<SoundObservation>>({});
	const posting = useRef(new Set<SpeedGroupId>());
	const retryAfter = useRef<SoundGroupMap<number>>({});

	useEffect(
		() => () => {
			analyzers.current.forEach(({ analyzer }) => {
				analyzer.stop();
			});
			analyzers.current.clear();
		},
		[],
	);

	const postObservation = useCallback(
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
						acceptState(await serverRef.current.observeSpeedGroup(group, next));
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

	useEffect(() => {
		for (const group of speedGroupIds) {
			const saved = states[group]?.configuration;
			const preview = previews[group];
			const configuration = preview ?? saved;
			const deviceId = deviceIds[group] ?? "";
			const shouldCapture = Boolean(
				configuration && deviceId && (saved?.enabled || preview),
			);
			const running = analyzers.current.get(group);
			if (!shouldCapture || !configuration) {
				if (running) {
					running.analyzer.stop();
					analyzers.current.delete(group);
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
				continue;
			}
			if (running?.deviceId === deviceId) {
				running.analyzer.updateConfiguration(configuration);
				continue;
			}
			running?.analyzer.stop();
			const analyzer = new SoundToLightAudioAnalyzer(
				configuration,
				(observation) => {
					if (shouldPublishSoundObservation(statesRef.current[group])) {
						postObservation(group, observation);
					}
				},
				(status) => {
					if (!mounted.current) return;
					setCaptures((current) => ({ ...current, [group]: status }));
					if (status.phase === "capturing") {
						setPermission("granted");
						if (!status.observation) void refreshInputs();
					}
					if (status.phase === "permission_denied") setPermission("denied");
				},
			);
			analyzers.current.set(group, { deviceId, analyzer });
			void analyzer.start(deviceId);
		}
	}, [
		deviceIds,
		mounted,
		postObservation,
		previews,
		refreshInputs,
		setPermission,
		states,
	]);

	return captures;
}
