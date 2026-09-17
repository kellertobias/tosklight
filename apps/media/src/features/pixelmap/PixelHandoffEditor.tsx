// The desk handoff of the selected zone, shown only in desk-merge mode.
//
// A handoff has more fields than fit in a table row, and only one zone is being patched at a time,
// so it opens as a form under the zone table for the selected row.

import {
	CheckboxField,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui/controls";
import type {
	PixelZoneHandoffView,
	PixelZoneView,
} from "../../shared/api/generated/media-wire";

export function PixelHandoffEditor({
	handoff,
	zone,
	deskShowName,
	onChange,
}: {
	handoff: PixelZoneHandoffView;
	zone: PixelZoneView;
	deskShowName?: string;
	onChange: (handoff: PixelZoneHandoffView) => void;
}) {
	const edit = (patch: Partial<PixelZoneHandoffView>) =>
		onChange({ ...handoff, ...patch });
	return (
		<fieldset
			className="media-pixel-zone-editor"
			aria-label={`${zone.name} desk handoff`}
		>
			<TextField
				label="Zone fixture"
				value={handoff.fixtureName}
				onChange={(event) => edit({ fixtureName: event.target.value })}
			/>
			<SelectField
				label="Desk input protocol"
				value={handoff.protocol}
				options={[
					{ value: "art-net", label: "Art-Net" },
					{ value: "sacn", label: "sACN" },
				]}
				onChange={(protocol) => edit({ protocol })}
			/>
			<NumberField
				label="Desk input universe"
				min={0}
				step={1}
				value={String(handoff.inputUniverse)}
				onChange={(event) =>
					edit({ inputUniverse: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Desk input first pixel address"
				min={1}
				max={512}
				step={1}
				value={String(handoff.inputStartAddress)}
				onChange={(event) =>
					edit({ inputStartAddress: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Dimmer address"
				min={1}
				max={512}
				step={1}
				value={String(handoff.dimmerAddress)}
				onChange={(event) =>
					edit({ dimmerAddress: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Mix address"
				min={1}
				max={512}
				step={1}
				value={String(handoff.mixAddress)}
				onChange={(event) => edit({ mixAddress: Number(event.target.value) })}
			/>
			<NumberField
				label="Zone fixture footprint"
				description="Channels after the mapped pixels pass through unchanged from the desk."
				min={zone.footprint}
				max={512}
				step={1}
				value={String(handoff.fixtureFootprint)}
				onChange={(event) =>
					edit({ fixtureFootprint: Number(event.target.value) })
				}
			/>
			<CheckboxField
				label="Request automatic desk patch"
				description={
					deskShowName
						? `ToskLight desk recognized (${deskShowName}); patching waits for an authenticated desk confirmation.`
						: "No compatible ToskLight desk is currently recognized; use the manual desk input patch above."
				}
				checked={handoff.automaticPatch}
				disabled={!deskShowName}
				onChange={(event) => edit({ automaticPatch: event.target.checked })}
			/>
		</fieldset>
	);
}
