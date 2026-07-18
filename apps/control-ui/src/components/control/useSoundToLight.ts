import { useCallback, useEffect, useRef, useState } from "react";
import { useServer } from "../../api/ServerContext";
import type {
	SoundToLightConfig,
	SpeedGroupActionInput,
	SpeedGroupId,
	SpeedGroupSoundState,
} from "../../api/types";
import type {
	AudioInputDevice,
	MicrophonePermission,
	SoundCaptureStatus,
} from "./soundToLightAnalyzer";
import {
	type SoundGroupMap,
	soundDeviceStorageKey,
	soundToLightErrorMessage,
	speedGroupIds,
} from "./soundToLightModel";
import { useSoundCapture } from "./useSoundCapture";
import { useSoundDeviceSelection } from "./useSoundDeviceSelection";

export { shouldPublishSoundObservation } from "./useSoundCapture";
export { soundDeviceStorageKey, speedGroupIds };

export interface SoundToLightController {
	states: SoundGroupMap<SpeedGroupSoundState>;
	captures: SoundGroupMap<SoundCaptureStatus>;
	devices: AudioInputDevice[];
	deviceIds: SoundGroupMap<string>;
	permission: MicrophonePermission;
	loading: boolean;
	error: string | null;
	setDevice: (group: SpeedGroupId, deviceId: string) => void;
	setPreview: (
		group: SpeedGroupId,
		configuration: SoundToLightConfig | null,
	) => void;
	refreshInputs: () => Promise<void>;
	save: (
		group: SpeedGroupId,
		configuration: SoundToLightConfig,
	) => Promise<SpeedGroupSoundState>;
	action: (
		group: SpeedGroupId,
		input: SpeedGroupActionInput,
	) => Promise<SpeedGroupSoundState>;
}

export function useSoundToLight(): SoundToLightController {
	const server = useServer();
	const serverRef = useRef(server);
	serverRef.current = server;
	const deskId = server.session?.desk.id ?? null;
	const sessionId = server.session?.session_id ?? null;
	const [states, setStates] = useState<SoundGroupMap<SpeedGroupSoundState>>({});
	const [previews, setPreviews] = useState<SoundGroupMap<SoundToLightConfig>>(
		{},
	);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const mounted = useRef(true);

	useEffect(
		() => () => {
			mounted.current = false;
		},
		[],
	);

	const {
		devices,
		deviceIds,
		permission,
		setPermission,
		refreshInputs,
		setDevice,
	} = useSoundDeviceSelection(deskId, mounted);

	useEffect(() => {
		if (!sessionId) {
			setStates({});
			return;
		}
		let cancelled = false;
		setLoading(true);
		void Promise.all(
			speedGroupIds.map((group) => serverRef.current.speedGroup(group)),
		)
			.then((loaded) => {
				if (cancelled) return;
				setStates(
					Object.fromEntries(
						loaded.map((state) => [state.group, state]),
					) as SoundGroupMap<SpeedGroupSoundState>,
				);
				setError(null);
			})
			.catch((reason) => {
				if (!cancelled) {
					setError(
						`Unable to load Speed Groups: ${soundToLightErrorMessage(reason)}`,
					);
				}
			})
			.finally(() => {
				if (!cancelled) setLoading(false);
			});
		return () => {
			cancelled = true;
		};
	}, [sessionId]);

	const acceptState = useCallback((state: SpeedGroupSoundState) => {
		if (!mounted.current) return state;
		setStates((current) => ({ ...current, [state.group]: state }));
		return state;
	}, []);

	const captures = useSoundCapture({
		states,
		previews,
		deviceIds,
		mounted,
		acceptState,
		setError,
		setPermission,
		refreshInputs,
	});

	const setPreview = useCallback(
		(group: SpeedGroupId, configuration: SoundToLightConfig | null) => {
			setPreviews((current) => {
				const next = { ...current };
				if (configuration) next[group] = configuration;
				else delete next[group];
				return next;
			});
		},
		[],
	);

	const save = useCallback(
		async (group: SpeedGroupId, configuration: SoundToLightConfig) => {
			try {
				const state = acceptState(
					await serverRef.current.updateSpeedGroup(group, configuration),
				);
				setError(null);
				return state;
			} catch (reason) {
				setError(
					`Unable to save Speed Group ${group}: ${soundToLightErrorMessage(reason)}`,
				);
				throw reason;
			}
		},
		[acceptState],
	);

	const action = useCallback(
		async (group: SpeedGroupId, input: SpeedGroupActionInput) => {
			try {
				const state = acceptState(
					await serverRef.current.speedGroupAction(group, input),
				);
				setError(null);
				return state;
			} catch (reason) {
				setError(
					`Speed Group ${group} action failed: ${soundToLightErrorMessage(reason)}`,
				);
				throw reason;
			}
		},
		[acceptState],
	);

	return {
		states,
		captures,
		devices,
		deviceIds,
		permission,
		loading,
		error,
		setDevice,
		setPreview,
		refreshInputs,
		save,
		action,
	};
}
