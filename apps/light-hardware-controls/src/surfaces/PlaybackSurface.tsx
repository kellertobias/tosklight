import { attachedPlaybackLayout } from "@tosklight/ui/control-surface-contracts";
import { PlaybackButton } from "./playback/PlaybackButton";
import type { Lamp, SendControl } from "../controller/types";
import { EncoderEmulation } from "./playback/EncoderEmulation";
import { Playback } from "./playback/Playback";

interface PlaybackSurfaceProps {
	page: number;
	topRowVisible: boolean;
	levels: Record<string, number>;
	lamps: Record<string, Lamp>;
	send: SendControl;
	nativeSimulator?: boolean;
}

export function PlaybackSurface({
	page,
	topRowVisible,
	levels,
	lamps,
	send,
	nativeSimulator = false,
}: PlaybackSurfaceProps) {
	return (
		<section
			className={`playback-surface ${topRowVisible ? "with-top-row" : "without-top-row"}`}
		>
			<div className="encoder-row">
				{attachedPlaybackLayout.encoderSlots.map((number) => (
					<EncoderEmulation key={number} number={number} send={send} />
				))}
				<EncoderEmulation
					number={attachedPlaybackLayout.navigationEncoder}
					nav
					send={send}
					clickDisabled={nativeSimulator}
				/>
			</div>
			<div className="top-row">
				{attachedPlaybackLayout.topSlots.map((slot) => (
					<PlaybackButton
						key={slot}
						page={page}
						position={slot}
						label={String(slot)}
						lamp={lamps[`${page}/${slot}/1`]}
						send={send}
					/>
				))}
			</div>
			<div className="playback-bank">
				{attachedPlaybackLayout.mainSlots.map((slot) => (
					<Playback
						key={slot}
						slot={slot}
						page={page}
						levels={levels}
						lamps={lamps}
						send={send}
					/>
				))}
			</div>
		</section>
	);
}
