import type { WallOpening } from "@tosklight/patch";
import { CadWallOpeningFields } from "./CadWallOpeningFields";
export function CadWallOpenings({
	openings,
	onChange,
}: {
	openings: WallOpening[];
	onChange(value: WallOpening[]): void;
}) {
	const change = (index: number, next: WallOpening) =>
		onChange(openings.map((value, i) => (i === index ? next : value)));
	const add = (door: boolean) =>
		onChange([
			...openings,
			{
				id: crypto.randomUUID(),
				offset_millimetres: openings.reduce(
					(end, o) =>
						Math.max(end, o.offset_millimetres + o.width_millimetres + 200),
					200,
				),
				sill_millimetres: door ? 0 : 1000,
				width_millimetres: door ? 900 : 1000,
				height_millimetres: door ? 2100 : 1000,
				opening: door
					? { kind: "door", leaves: "single", swing: "inward", hinge: "left" }
					: { kind: "window", shape: "rectangular" },
			},
		]);
	return (
		<section aria-label="Wall openings">
			<h3>Windows and doors</h3>
			<p>
				Offsets are measured from the wall's local start. Openings move with
				their wall.
			</p>
			{openings.map((opening, index) => (
				<fieldset key={opening.id}>
					<legend>
						{opening.opening.kind === "door" ? "Door" : "Window"} {index + 1}
					</legend>
					<CadWallOpeningFields
						opening={opening}
						index={index}
						onChange={(next) => change(index, next)}
					/>
					<button
						type="button"
						className="ui-button"
						onClick={() => onChange(openings.filter((_, i) => i !== index))}
					>
						Remove opening {index + 1}
					</button>
				</fieldset>
			))}
			<div className="cad-architecture-actions">
				<button type="button" className="ui-button" onClick={() => add(false)}>
					Add window
				</button>
				<button type="button" className="ui-button" onClick={() => add(true)}>
					Add door
				</button>
			</div>
		</section>
	);
}
