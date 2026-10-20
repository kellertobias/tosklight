import type { VenueArchitecture } from "@tosklight/patch";
import type { ArchitectureDraft } from "./architectureDraft";
import { CadWallOpenings } from "./CadWallOpenings";
import { CommitNumber, CommitText } from "./cadFields";

export function CadArchitectureFields({
	draft,
	onChange,
}: {
	draft: ArchitectureDraft;
	onChange(next: ArchitectureDraft): void;
}) {
	const architecture = draft.architecture;
	const set = (next: VenueArchitecture) =>
		onChange({ ...draft, architecture: next });
	const equal =
		(architecture.kind === "pillar" && architecture.shape !== "rectangular") ||
		(architecture.kind === "table" && architecture.shape === "round");
	return (
		<>
			<CommitText
				label="Name"
				value={draft.name}
				onCommit={(name) => onChange({ ...draft, name })}
			/>
			{architecture.kind !== "slab" ? (
				<fieldset className="cad-architecture-grid" aria-label="Dimensions">
					{(["x", "y", "z"] as const).map((axis) => (
						<CommitNumber
							key={axis}
							label={
								architecture.kind === "stairs"
									? ({ x: "Width", y: "Rise", z: "Run" } as const)[axis]
									: axis === "x"
										? architecture.kind === "wall"
											? "Length"
											: "Width"
										: axis === "y"
											? "Height"
											: architecture.kind === "wall"
												? "Thickness"
												: "Depth"
							}
							unit="m"
							min={0.001}
							value={draft.size[axis] / 1000}
							onCommit={(value) => {
								const size = {
									...draft.size,
									[axis]: Math.round(value * 1000),
								};
								if (equal && (axis === "x" || axis === "z")) {
									size.x = size[axis];
									size.z = size[axis];
								}
								onChange({ ...draft, size });
							}}
						/>
					))}
				</fieldset>
			) : null}
			<fieldset className="cad-architecture-grid" aria-label="Placement">
				{(["x", "y", "z"] as const).map((axis) => (
					<CommitNumber
						key={axis}
						label={`Position ${axis.toUpperCase()}`}
						unit="m"
						value={draft.position[axis] / 1000}
						onCommit={(value) =>
							onChange({
								...draft,
								position: {
									...draft.position,
									[axis]: Math.round(value * 1000),
								},
							})
						}
					/>
				))}
				<CommitNumber
					label="Angle"
					unit="°"
					digits={1}
					value={draft.angle}
					onCommit={(angle) => onChange({ ...draft, angle })}
				/>
			</fieldset>
			{architecture.kind === "wall" ? (
				<>
					<WallColour draft={draft} onChange={onChange} />
					<CommitNumber
						label="Room ceiling height"
						unit="m"
						min={0.001}
						value={
							(architecture.ceiling_height_millimetres ?? draft.size.y) / 1000
						}
						onCommit={(value) =>
							set({
								...architecture,
								ceiling_height_millimetres: Math.round(value * 1000),
							})
						}
					/>
					<p>
						Closed wall loops generate a ceiling. Wall position is its centre;
						length and angle determine both endpoints.
					</p>
					<CadWallOpenings
						openings={architecture.openings}
						onChange={(openings) => set({ ...architecture, openings })}
					/>
				</>
			) : null}
			{architecture.kind === "stairs" ? (
				<section aria-label="Stair construction">
					<CommitNumber
						label="Steps"
						digits={0}
						min={1}
						max={256}
						value={architecture.steps}
						onCommit={(value) =>
							set({ ...architecture, steps: Math.round(value) })
						}
					/>
					<p>
						Stairs rise along positive local Depth, towards the front
						(audience). Position Z is the lower floor datum. Width spans the
						stairs, Rise is the full height, and Run is the total horizontal
						depth. Intersecting floors and balconies receive an opening
						automatically.
					</p>
				</section>
			) : null}
			<ArchitectureOptions draft={draft} onChange={onChange} />
		</>
	);
}
function WallColour({
	draft,
	onChange,
}: {
	draft: ArchitectureDraft;
	onChange(next: ArchitectureDraft): void;
}) {
	return (
		<div className="cad-architecture-grid">
			<label className="cad-field">
				Wall colour
				<input
					type="color"
					aria-label="Wall colour"
					value={draft.colourSrgb}
					onChange={(event) =>
						onChange({ ...draft, colourSrgb: event.currentTarget.value })
					}
				/>
			</label>
			<CommitText
				label="Wall colour hex"
				value={draft.colourSrgb}
				accepts={(value) => /^#[0-9a-f]{6}$/i.test(value)}
				onCommit={(colourSrgb) => onChange({ ...draft, colourSrgb })}
			/>
		</div>
	);
}

function ArchitectureOptions({
	draft,
	onChange,
}: {
	draft: ArchitectureDraft;
	onChange(next: ArchitectureDraft): void;
}) {
	const architecture = draft.architecture;
	const set = (next: VenueArchitecture) =>
		onChange({ ...draft, architecture: next });
	return (
		<>
			{architecture.kind === "pillar" || architecture.kind === "table" ? (
				<label className="cad-field">
					Footprint
					<select
						className="ui-input"
						aria-label="Footprint"
						value={architecture.shape}
						onChange={(event) => {
							const shape = event.currentTarget
								.value as typeof architecture.shape;
							onChange({
								...draft,
								size:
									shape === "rectangular"
										? draft.size
										: { ...draft.size, z: draft.size.x },
								architecture: { ...architecture, shape } as VenueArchitecture,
							});
						}}
					>
						{(architecture.kind === "pillar"
							? ["round", "square", "rectangular"]
							: ["round", "rectangular"]
						).map((value) => (
							<option key={value}>{value}</option>
						))}
					</select>
				</label>
			) : null}
			{architecture.kind === "chair" ? (
				<label className="cad-field">
					Chair style
					<select
						className="ui-input"
						aria-label="Chair style"
						value={architecture.style}
						onChange={(event) =>
							set({
								...architecture,
								style: event.currentTarget.value as typeof architecture.style,
							})
						}
					>
						<option value="black_metal">Black with metal legs</option>
						<option value="white">Decorative white</option>
						<option value="bar_stool">Bar stool</option>
					</select>
				</label>
			) : null}
			{architecture.kind === "slab" ? (
				<SlabFields architecture={architecture} onChange={set} />
			) : null}
		</>
	);
}

function SlabFields({
	architecture,
	onChange,
}: {
	architecture: Extract<VenueArchitecture, { kind: "slab" }>;
	onChange(next: VenueArchitecture): void;
}) {
	return (
		<section aria-label="Slab outline">
			<label className="cad-field">
				Slab type
				<select
					className="ui-input"
					aria-label="Slab type"
					value={architecture.purpose}
					onChange={(event) =>
						onChange({
							...architecture,
							purpose: event.currentTarget.value as typeof architecture.purpose,
						})
					}
				>
					<option value="floor">Floor</option>
					<option value="roof">Roof</option>
					<option value="balcony">Balcony</option>
				</select>
			</label>
			<CommitNumber
				label="Elevation above floor"
				unit="m"
				value={architecture.elevation_millimetres / 1000}
				onCommit={(value) =>
					onChange({
						...architecture,
						elevation_millimetres: Math.round(value * 1000),
					})
				}
			/>
			<CommitNumber
				label="Slab thickness"
				unit="m"
				min={0.001}
				value={architecture.thickness_millimetres / 1000}
				onCommit={(value) =>
					onChange({
						...architecture,
						thickness_millimetres: Math.round(value * 1000),
					})
				}
			/>
			<p>
				Outline points use local X across and Depth towards the front
				(audience), in order around the edge. Duplicate a balcony and change its
				elevation to build stacked levels.
			</p>
			{architecture.outline_millimetres.map((point, index) => (
				<div className="cad-architecture-grid" key={index}>
					{(["x", "y"] as const).map((axis) => (
						<CommitNumber
							key={axis}
							label={`Point ${index + 1} ${axis === "y" ? "Depth" : "X"}`}
							unit="m"
							value={point[axis] / 1000}
							onCommit={(value) =>
								onChange({
									...architecture,
									outline_millimetres: architecture.outline_millimetres.map(
										(p, i) =>
											i === index
												? { ...p, [axis]: Math.round(value * 1000) }
												: p,
									),
								})
							}
						/>
					))}
					<button
						type="button"
						className="ui-button"
						disabled={architecture.outline_millimetres.length <= 3}
						onClick={() =>
							onChange({
								...architecture,
								outline_millimetres: architecture.outline_millimetres.filter(
									(_, i) => i !== index,
								),
							})
						}
					>
						Remove point {index + 1}
					</button>
				</div>
			))}
			<button
				type="button"
				className="ui-button"
				onClick={() => {
					const last = architecture.outline_millimetres.at(-1) ?? {
						x: 0,
						y: 0,
					};
					const first = architecture.outline_millimetres[0];
					onChange({
						...architecture,
						outline_millimetres: [
							...architecture.outline_millimetres,
							{
								x: Math.round((last.x + first.x) / 2),
								y: Math.round((last.y + first.y) / 2),
							},
						],
					});
				}}
			>
				Add outline point
			</button>
		</section>
	);
}
