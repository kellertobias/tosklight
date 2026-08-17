import { ModalLayer, ModalTitleBar } from "@tosklight/ui";
import {
	DEFAULT_POOL_CARD_MINIMUM_WIDTH,
	PoolCard,
	PoolGrid,
} from "@tosklight/ui/pools";
import type { TimecodeCueListOption } from "./TimecodeTimelineEditor";

export function CueListChooser({
	cueLists,
	value,
	onChange,
	onClose,
	onAdd,
}: {
	cueLists: readonly TimecodeCueListOption[];
	value: string;
	onChange(value: string): void;
	onClose(): void;
	onAdd(): void;
}) {
	return (
		<ModalLayer
			ariaLabel="Choose Cue List"
			dialogClassName="timecode-cuelist-chooser-modal"
			onClose={onClose}
		>
			<ModalTitleBar
				title="Choose Cue List"
				onClose={onClose}
				closeLabel="Cancel adding Cue List lane"
				accept={{
					id: "add",
					label: "Add lane",
					variant: "primary",
					disabled: !value || !cueLists.some((cueList) => cueList.id === value),
					onPress: onAdd,
				}}
			/>
			<div className="timecode-cuelist-chooser-scroll">
				<PoolGrid
					className="timecode-cuelist-chooser-grid"
					minimumCardWidth={DEFAULT_POOL_CARD_MINIMUM_WIDTH}
					fillEmptySlots={false}
					slots={cueLists.map((cueList, index) => ({
						id: cueList.id,
						position: index,
						card: {
							number: index + 1,
							primary: cueList.name,
							secondary: `${cueList.cues.length} ${cueList.cues.length === 1 ? "Cue" : "Cues"}`,
							kind: "cuelist" as const,
							states: cueList.id === value ? (["selected"] as const) : [],
						},
					}))}
					emptySlot={(index) => ({
						id: `empty-${index}`,
						position: index,
						card: { number: index + 1, primary: "Empty", states: ["empty"] },
					})}
					renderSlot={(slot, index) => (
						<PoolCard
							aria-label={`Cue List ${index + 1}: ${cueLists[index]?.name ?? "Unknown"}`}
							aria-pressed={slot.id === value}
							model={slot.card}
							onClick={() => onChange(String(slot.id))}
						/>
					)}
				/>
				{!cueLists.length && (
					<p className="timecode-cuelist-chooser-empty">
						No Cue Lists available.
					</p>
				)}
			</div>
		</ModalLayer>
	);
}
