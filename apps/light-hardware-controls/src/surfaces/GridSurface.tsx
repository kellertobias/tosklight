import { attachedPlaybackLayout } from "@tosklight/ui/control-surface-contracts";
import { PlaybackButton } from "./playback/PlaybackButton";
import type { Lamp, SendControl } from "../controller/types";
import { SpeedGroups } from "./grid/SpeedGroups";
import { Playback } from "./playback/Playback";

interface GridSurfaceProps {
	page: number;
	levels: Record<string, number>;
	lamps: Record<string, Lamp>;
	speedBpms: Record<string, number>;
	send: SendControl;
}

export function GridSurface({
	page,
	levels,
	lamps,
	speedBpms,
	send,
}: GridSurfaceProps) {
	return (
		<section className="grid-layout">
			<div className="button-grid">
				{attachedPlaybackLayout.gridButtonSlots.map((slot) => (
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
			<aside className="grid-sidebar">
				<section className="six">
					<h2>Playbacks 91–96</h2>
					{attachedPlaybackLayout.gridPlaybackSlots.map((slot) => (
						<Playback
							key={slot}
							slot={slot}
							page={page}
							buttons={1}
							levels={levels}
							lamps={lamps}
							send={send}
						/>
					))}
				</section>
				<SpeedGroups speedBpms={speedBpms} lamps={lamps} send={send} />
			</aside>
		</section>
	);
}
