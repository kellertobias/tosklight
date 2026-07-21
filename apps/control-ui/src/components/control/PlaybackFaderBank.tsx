import { memo } from "react";
import { PhysicalPlaybackConfigurationModal } from "./PhysicalPlaybackConfigurationModal";
import {
	type PlaybackFaderBankProps,
	usePlaybackBankController,
} from "./playbackFaderBank/controller";
import { PlaybackSlot } from "./playbackFaderBank/PlaybackSlot";

export const PlaybackFaderBank = memo<PlaybackFaderBankProps>(
	function PlaybackFaderBank(props: PlaybackFaderBankProps = {}) {
		const controller = usePlaybackBankController(props);
		if (!controller.authorityReady)
			return (
				<div
					className="playback-fader-bank playback-authority-status"
					role={controller.authorityError ? "alert" : "status"}
				>
					{controller.authorityError?.message ?? "Loading Playbacks…"}
				</div>
			);
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
					<PhysicalPlaybackConfigurationModal
						{...controller.configuration}
						onClose={() => controller.setConfiguration(null)}
					/>
				)}
			</>
		);
	},
	equalPlaybackFaderBankProps,
);

function equalPlaybackFaderBankProps(
	left: PlaybackFaderBankProps,
	right: PlaybackFaderBankProps,
) {
	return (
		(left.pageNumber ?? null) === (right.pageNumber ?? null) &&
		(left.firstSlot ?? 1) === (right.firstSlot ?? 1) &&
		(left.count ?? null) === (right.count ?? null) &&
		(left.rows ?? null) === (right.rows ?? null) &&
		(left.buttons ?? null) === (right.buttons ?? null) &&
		Boolean(left.hardwareConnected) === Boolean(right.hardwareConnected) &&
		equalPlaybackLayout(left.playbackLayout, right.playbackLayout)
	);
}

function equalPlaybackLayout(
	left: PlaybackFaderBankProps["playbackLayout"],
	right: PlaybackFaderBankProps["playbackLayout"],
) {
	if (left === right) return true;
	if (!left || !right || left.playbacks_per_row !== right.playbacks_per_row)
		return false;
	return (
		left.rows.length === right.rows.length &&
		left.rows.every((row, index) => {
			const candidate = right.rows[index];
			if (!candidate) return false;
			return (
				row.first_playback_slot === candidate.first_playback_slot &&
				row.has_fader === candidate.has_fader &&
				row.button_count === candidate.button_count
			);
		})
	);
}

export type { PlaybackFaderBankProps } from "./playbackFaderBank/controller";
export {
	emptyConfiguration,
	playbackButtonLabel,
} from "./playbackFaderBank/feedback";
export { playbackRowUnits } from "./playbackFaderBank/projection";
