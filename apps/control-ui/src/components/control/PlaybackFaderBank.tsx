import { PlaybackConfigurationModal } from "./PlaybackConfigurationModal";
import {
	type PlaybackFaderBankProps,
	usePlaybackBankController,
} from "./playbackFaderBank/controller";
import { PlaybackSlot } from "./playbackFaderBank/PlaybackSlot";

export function PlaybackFaderBank(props: PlaybackFaderBankProps = {}) {
	const controller = usePlaybackBankController(props);
	return (
		<>
			<div
				className={`playback-fader-bank ${controller.hardware ? "hardware-layout" : "touch-layout"}`}
				style={{
					gridTemplateColumns: `repeat(${controller.columns}, minmax(0, 1fr))`,
					gridTemplateRows: controller.rowTracks,
				}}
			>
				{controller.slots.map((slotData) => (
					<PlaybackSlot
						controller={controller}
						slotData={slotData}
						key={`${slotData.slot}-${slotData.playback?.number ?? "empty"}`}
					/>
				))}
			</div>
			{controller.configuration && (
				<PlaybackConfigurationModal
					playback={controller.configuration.playback}
					page={controller.configuration.page}
					slot={controller.configuration.slot}
					empty={controller.configuration.empty}
					onClose={() => controller.setConfiguration(null)}
				/>
			)}
		</>
	);
}

export type { PlaybackFaderBankProps } from "./playbackFaderBank/controller";
export {
	emptyConfiguration,
	playbackButtonLabel,
} from "./playbackFaderBank/feedback";
export { playbackRowUnits } from "./playbackFaderBank/projection";
