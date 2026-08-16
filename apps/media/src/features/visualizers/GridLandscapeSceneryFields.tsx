import { SelectField } from "@tosklight/ui/controls";

const SCENERY = [
	{ value: "0", label: "Off" },
	{ value: "1", label: "Street lamps" },
	{ value: "2", label: "Palm trees" },
];

export function GridLandscapeSceneryFields({
	left,
	right,
	onLeftChange,
	onRightChange,
}: {
	left: number;
	right: number;
	onLeftChange: (value: number) => void;
	onRightChange: (value: number) => void;
}) {
	return (
		<>
			<SelectField
				label="Left scenery"
				value={String(left)}
				options={SCENERY}
				onChange={(value) => onLeftChange(Number(value))}
			/>
			<SelectField
				label="Right scenery"
				value={String(right)}
				options={SCENERY}
				onChange={(value) => onRightChange(Number(value))}
			/>
		</>
	);
}
