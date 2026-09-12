import { useMemo } from "react";
import { Button } from "@tosklight/ui";
import { derivePrimarySlots } from "@tosklight/patch";
import type { FixtureProfile } from "@tosklight/patch";

/**
 * Manufacturer, then fixture, then what that fixture is — the order an operator narrows down in.
 *
 * Each column only ever lists what the column to its left has already chosen, so the list never
 * shows a fixture that belongs to a manufacturer you are not looking at.
 */

export function modeFootprint(mode: FixtureProfile["modes"][number]) {
	const footprint = mode.splits.reduce(
		(total, split) => total + split.footprint,
		0,
	);
	return footprint || derivePrimarySlots(mode).slots.size;
}

function ColumnList({
	label,
	children,
}: {
	label: string;
	children: React.ReactNode;
}) {
	return (
		<section className="viz-fixture-column" aria-label={label}>
			<h3>{label}</h3>
			<div className="viz-fixture-column-body">{children}</div>
		</section>
	);
}

export function FixtureLibraryBrowser({
	profiles,
	manufacturer,
	selected,
	onManufacturer,
	onSelect,
	onEdit,
}: {
	profiles: readonly FixtureProfile[];
	manufacturer: string | null;
	selected: FixtureProfile | null;
	onManufacturer: (manufacturer: string) => void;
	onSelect: (profile: FixtureProfile) => void;
	onEdit: (profile: FixtureProfile) => void;
}) {
	const manufacturers = useMemo(() => {
		const counts = new Map<string, number>();
		for (const profile of profiles)
			counts.set(
				profile.manufacturer,
				(counts.get(profile.manufacturer) ?? 0) + 1,
			);
		return [...counts.entries()].sort(([left], [right]) =>
			left.localeCompare(right),
		);
	}, [profiles]);

	const fixtures = useMemo(
		() =>
			profiles
				.filter((profile) => profile.manufacturer === manufacturer)
				.sort((left, right) => left.name.localeCompare(right.name)),
		[profiles, manufacturer],
	);

	return (
		<div className="viz-fixture-browser">
			<ColumnList label="Manufacturer">
				{manufacturers.length === 0 ? (
					<p className="empty-editor-message" role="status">
						No fixtures yet.
					</p>
				) : (
					<ul>
						{manufacturers.map(([name, count]) => (
							<li key={name}>
								<button
									type="button"
									aria-current={name === manufacturer}
									className={name === manufacturer ? "is-selected" : undefined}
									onClick={() => onManufacturer(name)}
								>
									<span>{name}</span>
									<small>{count}</small>
								</button>
							</li>
						))}
					</ul>
				)}
			</ColumnList>

			<ColumnList label="Fixture">
				{!manufacturer ? (
					<p className="empty-editor-message" role="status">
						Choose a manufacturer.
					</p>
				) : (
					<ul>
						{fixtures.map((profile) => (
							<li key={profile.id}>
								<button
									type="button"
									aria-current={profile.id === selected?.id}
									className={
										profile.id === selected?.id ? "is-selected" : undefined
									}
									onClick={() => onSelect(profile)}
								>
									<span>{profile.name}</span>
									<small>{profile.modes.length} modes</small>
								</button>
							</li>
						))}
					</ul>
				)}
			</ColumnList>

			<ColumnList label="Fixture info">
				{!selected ? (
					<p className="empty-editor-message" role="status">
						Choose a fixture to see what it is.
					</p>
				) : (
					<FixtureInfo profile={selected} onEdit={onEdit} />
				)}
			</ColumnList>
		</div>
	);
}

function Fact({ label, value }: { label: string; value: string | null }) {
	if (!value) return null;
	return (
		<>
			<dt>{label}</dt>
			<dd>{value}</dd>
		</>
	);
}

function metres(millimetres: number | null | undefined) {
	return millimetres == null ? null : `${millimetres} mm`;
}

function FixtureInfo({
	profile,
	onEdit,
}: {
	profile: FixtureProfile;
	onEdit: (profile: FixtureProfile) => void;
}) {
	const physical = profile.physical ?? {};
	const optics = profile.optics ?? {};
	const size = [
		physical.width_millimetres,
		physical.height_millimetres,
		physical.depth_millimetres,
	];
	return (
		<article className="viz-fixture-info">
			{profile.photograph_asset ? (
				<img src={profile.photograph_asset} alt={`${profile.name}`} />
			) : null}
			<h4>{profile.name}</h4>
			<p className="viz-fixture-info-identity">
				{profile.manufacturer}
				{profile.short_name ? ` · ${profile.short_name}` : ""} · revision{" "}
				{profile.revision}
			</p>
			<dl>
				<Fact label="Type" value={profile.fixture_type ?? null} />
				<Fact
					label="Modes"
					value={profile.modes
						.map((mode) => `${mode.name} (${modeFootprint(mode)})`)
						.join(", ")}
				/>
				<Fact
					label="Size"
					value={
						size.every((value) => value != null) ? size.join(" × ") + " mm" : null
					}
				/>
				<Fact
					label="Weight"
					value={
						physical.weight_kilograms == null
							? null
							: `${physical.weight_kilograms} kg`
					}
				/>
				<Fact
					label="Power"
					value={physical.power_watts == null ? null : `${physical.power_watts} W`}
				/>
				<Fact label="Connectors" value={physical.connectors || null} />
				<Fact label="Light source" value={physical.light_source || null} />
				<Fact label="Lens" value={physical.lens || null} />
				<Fact
					label="Colour temperature"
					value={
						optics.color_temperature_kelvin == null
							? null
							: `${optics.color_temperature_kelvin} K`
					}
				/>
				<Fact
					label="Luminous output"
					value={
						optics.luminous_output_lumens == null
							? null
							: `${optics.luminous_output_lumens} lm`
					}
				/>
				<Fact
					label="Beam angle"
					value={
						optics.beam_angle_degrees == null
							? null
							: `${optics.beam_angle_degrees}°`
					}
				/>
				<Fact
					label="Light source size"
					value={
						optics.light_source
							? `${optics.light_source.form} · ${metres(
									optics.light_source.width_millimetres,
								)} × ${metres(optics.light_source.height_millimetres)}`
							: null
					}
				/>
			</dl>
			{profile.notes ? (
				<p className="viz-fixture-info-notes">{profile.notes}</p>
			) : null}
			<Button onClick={() => onEdit(profile)}>Edit as new revision</Button>
		</article>
	);
}
