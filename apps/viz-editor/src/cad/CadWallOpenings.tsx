import type { WallOpening } from "@tosklight/patch";
import { CommitNumber } from "./cadFields";
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
					<div className="cad-architecture-grid">
						{(
							[
								"offset",
								"width",
								"height",
								...(opening.opening.kind === "window" ? ["sill"] : []),
							] as const
						).map((key) => (
							<CommitNumber
								key={key}
								label={key.charAt(0).toUpperCase() + key.slice(1)}
								ariaLabel={`Opening ${index + 1} ${key}`}
								unit="m"
								min={key === "offset" || key === "sill" ? 0 : 0.001}
								value={
									opening[`${key}_millimetres` as "width_millimetres"] / 1000
								}
								onCommit={(value) => {
									const next = {
										...opening,
										[`${key}_millimetres`]: Math.round(value * 1000),
									};
									if (
										opening.opening.kind === "window" &&
										opening.opening.shape === "round" &&
										(key === "width" || key === "height")
									) {
										next.width_millimetres = Math.round(value * 1000);
										next.height_millimetres = Math.round(value * 1000);
									}
									change(index, next);
								}}
							/>
						))}
						{opening.opening.kind === "window" ? (
							<label className="cad-field">
								Shape
								<select
									className="ui-input"
									aria-label={`Window ${index + 1} shape`}
									value={opening.opening.shape}
									onChange={(event) => {
										const shape = event.currentTarget.value as
											| "rectangular"
											| "round"
											| "arched";
										change(index, {
											...opening,
											height_millimetres:
												shape === "round"
													? opening.width_millimetres
													: opening.height_millimetres,
											opening: { kind: "window", shape },
										});
									}}
								>
									{["rectangular", "round", "arched"].map((value) => (
										<option key={value}>{value}</option>
									))}
								</select>
							</label>
						) : (
							(
								[
									["leaves", ["single", "double"]],
									["swing", ["inward", "outward"]],
									["hinge", ["left", "right"]],
								] as const
							).map(([key, values]) => (
								<label key={key} className="cad-field">
									{key}
									<select
										className="ui-input"
										aria-label={`Door ${index + 1} ${key}`}
										value={
											opening.opening.kind === "door"
												? opening.opening[key]
												: ""
										}
										onChange={(event) =>
											change(index, {
												...opening,
												opening: {
													...opening.opening,
													[key]: event.currentTarget.value,
												} as WallOpening["opening"],
											})
										}
									>
										{values.map((value) => (
											<option key={value}>{value}</option>
										))}
									</select>
								</label>
							))
						)}
					</div>
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
