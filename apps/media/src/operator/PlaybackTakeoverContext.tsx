import { SwitchField } from "@tosklight/ui/controls";
import {
	createContext,
	type ReactNode,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";
import { createPortal } from "react-dom";
import { MediaErrorToast, useFailureToast } from "../app/ToastContext";
import type { OutputView } from "../shared/api/generated/media-wire";
import {
	type LayerControl,
	useLayerControl,
	useOutputsForControl,
} from "../shared/api/layerControl";
import type { Resource } from "../shared/api/resource";

interface PlaybackTakeoverValue {
	outputs: Resource<OutputView[]>;
	control: LayerControl;
	selectedOutputId: string;
	selectOutput: (outputId: string) => void;
}

const PlaybackTakeoverContext = createContext<PlaybackTakeoverValue | null>(
	null,
);

export function PlaybackTakeoverProvider({
	children,
}: {
	children: ReactNode;
}) {
	const outputs = useOutputsForControl();
	const control = useLayerControl();
	const [selectedOutputId, selectOutput] = useState("");
	useFailureToast(control.refusal);

	useEffect(() => {
		const available = outputs.data ?? [];
		if (available.some((output) => output.id === selectedOutputId)) return;
		selectOutput(available[0]?.id ?? "");
	}, [outputs.data, selectedOutputId]);

	const value = useMemo(
		() => ({ outputs, control, selectedOutputId, selectOutput }),
		[outputs, control, selectedOutputId],
	);
	return (
		<PlaybackTakeoverContext.Provider value={value}>
			{children}
		</PlaybackTakeoverContext.Provider>
	);
}

/** Keeps feature tests and standalone stories usable without creating a second provider in App. */
export function PlaybackTakeoverBoundary({
	children,
}: {
	children: ReactNode;
}) {
	const inherited = useContext(PlaybackTakeoverContext);
	if (inherited) return children;
	return (
		<PlaybackTakeoverProvider>
			{children}
			<StandalonePlaybackTakeover />
		</PlaybackTakeoverProvider>
	);
}

function StandalonePlaybackTakeover() {
	const { control } = usePlaybackTakeover();
	const toggle = <PlaybackTakeoverToggle />;
	const dock = document.getElementById("media-playback-dock-action");
	return (
		<>
			{dock ? createPortal(toggle, dock) : toggle}
			{control.refusal && (
				<MediaErrorToast
					message={control.refusal.message}
					onDismiss={control.dismissRefusal}
				/>
			)}
		</>
	);
}

export function usePlaybackTakeover(): PlaybackTakeoverValue {
	const value = useContext(PlaybackTakeoverContext);
	if (!value)
		throw new Error("Playback takeover controls require their provider");
	return value;
}

export function PlaybackTakeoverToggle() {
	const { outputs, control, selectedOutputId } = usePlaybackTakeover();
	const output =
		outputs.data?.find((candidate) => candidate.id === selectedOutputId) ??
		outputs.data?.[0];
	return (
		<div className="media-playback-takeover-dock">
			<SwitchField
				bare
				className="media-playback-takeover"
				label="Take over playback"
				offLabel={null}
				onLabel={null}
				checked={output?.playbackTakeover ?? false}
				disabled={!output}
				onChange={(event) => {
					if (output) void control.setTakeover(output, event.target.checked);
				}}
			/>
		</div>
	);
}
