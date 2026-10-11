import { useRef } from "react";
import {
	controlSurfaceOscPaths,
	attachedPlaybackControlId,
} from "@tosklight/ui/control-surface-contracts";
import { ControlButton } from "../../components/ControlButton";
import type { Lamp, SendControl } from "../../controller/types";
export function PlaybackButton({
	page,
	position,
	number = 1,
	className = "",
	label = String(number),
	lamp,
	send,
}: {
	page: number;
	position: number;
	number?: 1 | 2 | 3;
	className?: string;
	label?: string;
	lamp?: Lamp;
	send: SendControl;
}) {
	const held = useRef<{ path: string; controlId: number } | null>(null);
	return (
		<ControlButton
			className={className}
			label={label}
			lamp={lamp}
			onDown={() => {
				if (held.current) return;
				const edge = {
					path: controlSurfaceOscPaths.explicitPlaybackControl(
						page,
						position,
						`button/${number}`,
					),
					controlId: attachedPlaybackControlId(position, `button/${number}`),
				};
				held.current = edge;
				send(edge.path, [true, edge.controlId]);
			}}
			onUp={() => {
				const edge = held.current;
				if (!edge) return;
				held.current = null;
				send(edge.path, [false, edge.controlId]);
			}}
		/>
	);
}
