import "./manualOptics.css";
import { NumberField } from "@tosklight/ui";
import type { AppearanceDraft } from "./lightSourceAppearanceModel";

export function ManualOpticsFields({
	draft,
	update,
}: {
	draft: AppearanceDraft;
	update: (change: Partial<AppearanceDraft>) => void;
}) {
	return (
		<fieldset className="manual-optics-fields">
			<legend>Manual optics</legend>
			<p>
				Installed adjustments apply only when zoom or framing blades have no live
				control channel.
			</p>
			<NumberField
				label="Manual zoom full opening (°)"
				description="Leave empty to inherit the profile. Full opening: 1°–170°."
				min={1}
				max={170}
				step={0.1}
				value={draft.manualZoomDegrees}
				onChange={(event) =>
					update({
						manualZoomDegrees: event.target.value,
						manualOpticsEdited: true,
					})
				}
			/>
			{draft.manualShaperBlades.map((value, index) => (
				<NumberField
					key={index}
					label={`Manual blade ${index + 1} closure (%)`}
					description="0% is open; 100% is fully inserted."
					min={0}
					max={100}
					step={0.1}
					value={value}
					onChange={(event) => {
						const blades = [...draft.manualShaperBlades] as AppearanceDraft["manualShaperBlades"];
						blades[index] = event.target.value;
						update({ manualShaperBlades: blades, manualOpticsEdited: true });
					}}
				/>
			))}
		</fieldset>
	);
}
