import type { WallOpening } from "@tosklight/patch";
import { CommitNumber } from "./cadFields";

export function CadWallOpeningFields({ opening, index = 0, onChange }: {
	opening: WallOpening;
	index?: number;
	onChange(next: WallOpening): void;
}) {
	const change = (_index: number, next: WallOpening) => onChange(next);
	return (
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
					key={{ leaves: "Door leaves", swing: "Swing", hinge: "Hinge side" }[key]}
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
						{{ leaves: "Door leaves", swing: "Swing", hinge: "Hinge side" }[key]}
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
	);
}
