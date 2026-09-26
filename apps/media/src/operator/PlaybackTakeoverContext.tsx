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
import { type LibraryPreview, type PreviewTarget, useLibraryPreview } from "./libraryPreview";

interface PlaybackTakeoverValue {
	outputs: Resource<OutputView[]>;
	control: LayerControl;
	selectedOutputId: string;
	selectOutput: (outputId: string) => void;
	preview: LibraryPreview;
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
	const preview = useLibraryPreview(outputs.data, control, selectedOutputId);
	useFailureToast(control.refusal);

	useEffect(() => {
		const available = outputs.data ?? [];
		if (available.some((output) => output.id === selectedOutputId)) return;
		selectOutput(available[0]?.id ?? "");
	}, [outputs.data, selectedOutputId]);

	const value = useMemo(
		() => ({ outputs, control, selectedOutputId, selectOutput, preview }),
		[outputs, control, selectedOutputId, preview],
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

/** The Library's preview, or null outside the provider (feature tests and stories). */
export function useOptionalLibraryPreview(): LibraryPreview | null {
	return useContext(PlaybackTakeoverContext)?.preview ?? null;
}

/**
 * Shows what an editor has selected on the preview while preview is on: again whenever the
 * selection changes, and at once when preview is turned on with something already selected.
 */
export function usePreviewOf(target: PreviewTarget | null) {
	const preview = useOptionalLibraryPreview();
	const on = preview?.outputId ?? null;
	const key = JSON.stringify(target);
	// biome-ignore lint/correctness/useExhaustiveDependencies: the target is compared by value, and the preview object changes identity whenever it re-renders.
	useEffect(() => {
		if (on && target) void preview?.show(target);
	}, [key, on]);
}

/** Take over playback, and on the pages that preview (Library and the editors) Enable preview beside it. */
export function PlaybackTakeoverToggle({ preview = false }: { preview?: boolean }) {
	const { outputs, control, selectedOutputId, preview: libraryPreview } =
		usePlaybackTakeover();
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
			{preview && (
				<SwitchField
					bare
					className="media-playback-takeover media-library-preview"
					label="Enable preview"
					offLabel={null}
					onLabel={null}
					checked={libraryPreview.outputId !== null}
					disabled={!output && libraryPreview.outputId === null}
					onChange={(event) => void libraryPreview.setEnabled(event.target.checked)}
				/>
			)}
			<small role="status">
				{output?.playbackTakeover ? "Web playback control active" : "Web playback control off"}
			</small>
		</div>
	);
}
