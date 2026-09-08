import type { TitleAction } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import type {
	TimecodeTransportSnapshot,
	TimecodeTransportAction,
} from "../../api/types/timecode";

export function useTimecodeEditorTransport(
	snapshot?: TimecodeTransportSnapshot,
) {
	const [result, setResult] = useState<TimecodeTransportSnapshot>();
	const transport =
		result && (!snapshot || result.revision > snapshot.revision)
			? result
			: snapshot;
	const [editorFrame, setEditorFrame] = useState(snapshot?.frame ?? 0);
	const [followTransport, setFollowTransport] = useState(true);
	const interactionRevision = useRef(0);
	useEffect(() => {
		if (followTransport && transport) setEditorFrame(transport.frame);
	}, [transport?.frame, followTransport]);
	return {
		transport,
		editorFrame,
		scrub(frame: number) {
			interactionRevision.current += 1;
			setFollowTransport(false);
			setEditorFrame(frame);
		},
		beginTransportAction() {
			return interactionRevision.current;
		},
		acceptTransportResponse(
			next?: TimecodeTransportSnapshot,
			follow = false,
			revision?: number,
		) {
			if (next) setResult(next);
			if (follow && revision === interactionRevision.current)
				setFollowTransport(true);
		},
	};
}

export function timecodeTransportActions({
	disabled,
	stopDisabled,
	state,
	onAction,
}: {
	disabled: boolean;
	stopDisabled: boolean;
	state?: TimecodeTransportSnapshot["state"];
	onAction(action: TimecodeTransportAction): Promise<void>;
}): TitleAction[] {
	return [
		{
			id: "rewind",
			label: (
				<span className="timecode-rewind-glyph" aria-hidden="true">
					<span>▏</span>
					<span className="timecode-reversed-play">▶</span>
				</span>
			),
			ariaLabel: "Rewind to start",
			onPress: () => {
				void onAction({ type: "seek", frame: 0 });
			},
			disabled: disabled,
			className: "timecode-transport-action",
		},
		{
			id: "stop",
			label: <span aria-hidden="true">■</span>,
			ariaLabel: "Stop",
			onPress: () => void onAction({ type: "stop" }),
			disabled: stopDisabled,
			className: "timecode-transport-action",
		},
		{
			id: "play",
			label: <span aria-hidden="true">▶</span>,
			ariaLabel: "Play",
			active: state === "playing",
			onPress: () => void onAction({ type: "go" }),
			disabled: disabled,
			className: "timecode-transport-action",
		},
		{
			id: "pause",
			label: <span aria-hidden="true">Ⅱ</span>,
			ariaLabel: "Pause",
			active: state === "paused",
			onPress: () => void onAction({ type: "pause" }),
			disabled: disabled,
			className: "timecode-transport-action",
		},
	];
}
