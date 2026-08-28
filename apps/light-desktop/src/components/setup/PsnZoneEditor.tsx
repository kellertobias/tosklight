import { Button, NumberField, SelectField, TextField } from "@tosklight/ui";
import type { PsnMacro, PsnZone } from "../../api/client/psn";

/**
 * The boxes on stage, and what walking into one does.
 *
 * A zone runs Macros, so the vocabulary is the one the operator already has: whatever they can
 * write on the command line, a zone can do. Leaving the leave Macro unset is how a zone that
 * should not turn itself off again is configured, so "None" is a real answer rather than a gap.
 */
export function PsnZoneEditor({
	zones,
	macros,
	occupiedZoneIds,
	busy,
	onChange,
}: {
	zones: PsnZone[];
	macros: PsnMacro[];
	occupiedZoneIds: string[];
	busy: boolean;
	onChange: (zones: PsnZone[]) => void;
}) {
	const replace = (zone: PsnZone) =>
		onChange(
			zones.map((candidate) => (candidate.id === zone.id ? zone : candidate)),
		);
	const macroOptions = [
		{ value: "", label: "None" },
		...macros.map((entry) => ({
			value: entry.id,
			label: `${entry.number} · ${entry.name}`,
		})),
	];
	return (
		<div className="psn-zones">
			{zones.map((zone) => (
				<fieldset
					key={zone.id}
					className={occupiedZoneIds.includes(zone.id) ? "is-occupied" : ""}
				>
					<legend>
						{zone.name || "Unnamed zone"}
						{occupiedZoneIds.includes(zone.id) && " — occupied"}
					</legend>
					<TextField
						label="Name"
						defaultValue={zone.name}
						key={`name-${zone.id}-${zone.name}`}
						onBlur={(event) => replace({ ...zone, name: event.target.value })}
					/>
					{(["x", "y", "z"] as const).map((axis, index) => (
						<NumberField
							key={`min-${zone.id}-${axis}`}
							label={`From ${axis} (m)`}
							defaultValue={zone.minMetres[index]}
							onBlur={(event) => {
								const minMetres = [...zone.minMetres] as PsnZone["minMetres"];
								minMetres[index] = Number(event.target.value);
								replace({ ...zone, minMetres });
							}}
						/>
					))}
					{(["x", "y", "z"] as const).map((axis, index) => (
						<NumberField
							key={`max-${zone.id}-${axis}`}
							label={`To ${axis} (m)`}
							defaultValue={zone.maxMetres[index]}
							onBlur={(event) => {
								const maxMetres = [...zone.maxMetres] as PsnZone["maxMetres"];
								maxMetres[index] = Number(event.target.value);
								replace({ ...zone, maxMetres });
							}}
						/>
					))}
					<SelectField
						label="On entering"
						value={zone.enterMacroId ?? ""}
						options={macroOptions}
						disabled={busy}
						onChange={(value) =>
							replace({ ...zone, enterMacroId: value || null })
						}
					/>
					<SelectField
						label="On leaving"
						value={zone.leaveMacroId ?? ""}
						options={macroOptions}
						disabled={busy}
						onChange={(value) =>
							replace({ ...zone, leaveMacroId: value || null })
						}
					/>
					<NumberField
						label="Hold for (ms)"
						description="How long somebody has to be in or out before it counts. Keeps a marker on the boundary from firing the Macro over and over."
						defaultValue={zone.dwellMillis}
						key={`dwell-${zone.id}-${zone.dwellMillis}`}
						onBlur={(event) =>
							replace({ ...zone, dwellMillis: Number(event.target.value) })
						}
					/>
					<Button
						disabled={busy}
						onClick={() =>
							onChange(zones.filter((candidate) => candidate.id !== zone.id))
						}
						aria-label={`Delete zone ${zone.name || "unnamed"}`}
					>
						Delete zone
					</Button>
				</fieldset>
			))}
			<Button
				disabled={busy}
				onClick={() =>
					onChange([
						...zones,
						{
							id: crypto.randomUUID(),
							name: `Zone ${zones.length + 1}`,
							minMetres: [-1, 0, -1],
							maxMetres: [1, 3, 1],
							trackerIds: [],
							enterMacroId: null,
							leaveMacroId: null,
							dwellMillis: 250,
						},
					])
				}
			>
				Add zone
			</Button>
		</div>
	);
}
