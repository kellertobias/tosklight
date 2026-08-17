import {
	Button,
	ModalLayer,
	ModalTitleBar,
	SelectionCardContent,
} from "@tosklight/ui";

export const TIMECODE_SPEED_GROUPS = ["A", "B", "C", "D", "E"] as const;

export function TimecodeSpeedGroupChooser({
	available,
	value,
	onChange,
	onClose,
	onAdd,
}: {
	available: readonly string[];
	value: string;
	onChange(value: string): void;
	onClose(): void;
	onAdd(): void;
}) {
	return (
		<ModalLayer
			ariaLabel="Choose Speed Group"
			className="ui-grouped-selection-layer"
			dialogClassName="ui-grouped-selection-modal timecode-speed-group-chooser"
			onClose={onClose}
		>
			<ModalTitleBar
				title="Choose Speed Group"
				onClose={onClose}
				closeLabel="Cancel adding Speed Group lane"
				accept={{
					id: "add",
					label: "Add lane",
					variant: "primary",
					disabled: !value || !available.includes(value),
					onPress: onAdd,
				}}
			/>
			<div className="ui-grouped-selection-groups">
				<section>
					<h3>Speed Group</h3>
					<div className="ui-grouped-selection-options">
						{available.map((group) => (
							<Button
								key={group}
								active={group === value}
								aria-pressed={group === value}
								contentAlign="left"
								onClick={() => onChange(group)}
							>
								<SelectionCardContent
									label={`Speed Group ${group}`}
									description={`Use desk Speed Group ${group} for this lane.`}
								/>
							</Button>
						))}
					</div>
				</section>
			</div>
		</ModalLayer>
	);
}
