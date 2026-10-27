import { CueListChooser } from "./TimecodeCueListChooser";
import { TimecodeSpeedGroupChooser } from "./TimecodeSpeedGroupChooser";
import type { TimecodeCueListOption } from "./timecodeEditorShared";

export function TimelineLaneChoosers({
	cueListChooserOpen,
	cueLists,
	cueListId,
	setCueListId,
	setCueListChooserOpen,
	addCueListLane,
	speedGroupChooserOpen,
	availableSpeedGroups,
	speedGroup,
	setSpeedGroup,
	setSpeedGroupChooserOpen,
	addSpeedLane,
}: {
	cueListChooserOpen: boolean;
	cueLists: readonly TimecodeCueListOption[];
	cueListId: string;
	setCueListId(value: string): void;
	setCueListChooserOpen(value: boolean): void;
	addCueListLane(id: string): void;
	speedGroupChooserOpen: boolean;
	availableSpeedGroups: string[];
	speedGroup: string;
	setSpeedGroup(value: string): void;
	setSpeedGroupChooserOpen(value: boolean): void;
	addSpeedLane(group: string): void;
}) {
	return (
		<>
			{cueListChooserOpen && (
				<CueListChooser
					cueLists={cueLists}
					value={cueListId}
					onChange={setCueListId}
					onClose={() => setCueListChooserOpen(false)}
					onAdd={() => {
						if (cueListId) addCueListLane(cueListId);
						setCueListChooserOpen(false);
					}}
				/>
			)}
			{speedGroupChooserOpen && (
				<TimecodeSpeedGroupChooser
					available={availableSpeedGroups}
					value={speedGroup}
					onChange={setSpeedGroup}
					onClose={() => setSpeedGroupChooserOpen(false)}
					onAdd={() => {
						if (speedGroup) addSpeedLane(speedGroup);
						setSpeedGroupChooserOpen(false);
					}}
				/>
			)}
		</>
	);
}
