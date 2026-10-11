import {
	controlSurfaceOscPaths,
	attachedPlaybackControlId,
} from "@tosklight/ui/control-surface-contracts";
import { memo } from "react";
import { PlaybackButton } from "./PlaybackButton";
import { TouchFader } from "../../components/TouchFader";
import type { Lamp, SendControl } from "../../controller/types";

interface PlaybackProps {
	slot: number;
	page: number;
	levels: Record<string, number>;
	lamps: Record<string, Lamp>;
	send: SendControl;
	buttons?: 1 | 3;
}

function PlaybackView({
	slot,
	page,
	levels,
	lamps,
	send,
	buttons = 3,
}: PlaybackProps) {
	const button = (number: 1 | 2 | 3, className = "") => (
		<PlaybackButton
			page={page}
			position={slot}
			number={number}
			className={className}
			lamp={lamps[`${page}/${slot}/${number}`]}
			send={send}
		/>
	);
	const level = levels[`${page}/${slot}`] ?? 0;

	return (
		<article className={`playback buttons-${buttons}`}>
			<b>{slot}</b>
			{button(1, "playback-button-one")}
			<TouchFader
				className="playback-touch-fader"
				label="FADER"
				value={level}
				display={`${Math.round(level * 100)}%`}
				onChange={(value) =>
					send(
						controlSurfaceOscPaths.explicitPlaybackControl(page, slot, "fader"),
						[value, attachedPlaybackControlId(slot, "fader")],
					)
				}
			/>
			{buttons === 3 && (
				<footer>
					{button(2)}
					{button(3)}
				</footer>
			)}
		</article>
	);
}

function sameLamp(left: Lamp | undefined, right: Lamp | undefined): boolean {
	return (
		left?.color === right?.color &&
		left?.state === right?.state &&
		left?.bpm === right?.bpm
	);
}

function samePlayback(previous: PlaybackProps, next: PlaybackProps): boolean {
	if (
		previous.slot !== next.slot ||
		previous.page !== next.page ||
		previous.buttons !== next.buttons ||
		previous.send !== next.send ||
		previous.levels[`${previous.page}/${previous.slot}`] !==
			next.levels[`${next.page}/${next.slot}`]
	) {
		return false;
	}
	const buttonCount = previous.buttons ?? 3;
	return Array.from(
		{ length: buttonCount },
		(_, index) => (index + 1) as 1 | 2 | 3,
	).every((button) =>
		sameLamp(
			previous.lamps[`${previous.page}/${previous.slot}/${button}`],
			next.lamps[`${next.page}/${next.slot}/${button}`],
		),
	);
}

export const Playback = memo(PlaybackView, samePlayback);
