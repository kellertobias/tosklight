import { FixtureColorDot } from "../components/shared/FixtureColorDot";
import { SourceValue } from "../components/shared/SourceValue";
import type { DataTableColumn } from "../components/window-kit";
import type { FixtureSheetRow } from "./fixtureSheetProjection";
import type { FixtureStepPresenter } from "./fixtureSheetStep";

type Column = DataTableColumn<FixtureSheetRow>;

function stepMarker(fixture: FixtureSheetRow, present: FixtureStepPresenter) {
	const presentation = present(fixture);
	if (presentation.current) return "STEP";
	if (presentation.containedCurrent) return "STEP INSIDE";
	if (presentation.base) return "BASE";
	if (presentation.containedBase) return "BASE INSIDE";
	return null;
}

function fixtureIdColumn(present: FixtureStepPresenter): Column {
	return {
		id: "id",
		header: "ID",
		width: "88px",
		render: (fixture) => {
			const marker = stepMarker(fixture, present);
			return (
				<span className="fixture-sheet-id">
					<span>{fixture.id}</span>
					{marker && <small className="fixture-step-marker">{marker}</small>}
				</span>
			);
		},
	};
}

function fixtureIconColumn(): Column {
	return {
		id: "icon",
		header: "Icon",
		width: "52px",
		align: "center",
		render: (fixture) => (
			<span className="fixture-sheet-icon">
				{fixture.icon ? (
					<img src={fixture.icon} alt="" />
				) : (
					<span title="No fixture icon">—</span>
				)}
			</span>
		),
	};
}

function fixtureNameColumn(showType: boolean): Column {
	return {
		id: "name",
		header: showType ? "Name / type" : "Name",
		width: "minmax(190px,1.4fr)",
		render: (fixture) => (
			<span className="fixture-name">
				<b>{fixture.name}</b>
				{showType && (
					<small className="fixture-type">{fixture.fixtureType}</small>
				)}
				{fixture.limitingGroups.length > 0 && (
					<em title={groupMasterTitle(fixture)}>
						◒ Group master {limitingGroupPercentage(fixture)}%
					</em>
				)}
			</span>
		),
	};
}

function groupMasterTitle(fixture: FixtureSheetRow) {
	return fixture.limitingGroups
		.map(
			(group) =>
				`${group.body.name}: ${Math.round((group.body.master ?? 1) * 100)}%`,
		)
		.join(", ");
}

function limitingGroupPercentage(fixture: FixtureSheetRow) {
	return Math.round(
		Math.max(...fixture.limitingGroups.map((group) => group.body.master ?? 1)) *
			100,
	);
}

function patchColumn(): Column {
	return {
		id: "patch",
		header: "Patch",
		width: "minmax(90px,.65fr)",
		render: (fixture) => (
			<span className="fixture-sheet-patch">{fixture.patch}</span>
		),
	};
}

function dimmerColumn(): Column {
	return {
		id: "dimmer",
		header: "Dimmer",
		width: "minmax(95px,.7fr)",
		render: (fixture) => (
			<SourceValue source={fixture.sources.dimmer}>
				<i className="vertical-meter">
					<i style={{ height: `${fixture.dimmer}%` }} />
				</i>
				{fixture.dimmer}%
				{fixture.preloadDimmer != null && (
					<small className="preload-value">→ {fixture.preloadDimmer}%</small>
				)}
			</SourceValue>
		),
	};
}

function colorColumn(): Column {
	return {
		id: "color",
		header: "Color",
		width: "minmax(105px,1fr)",
		render: (fixture) => (
			<SourceValue source={fixture.sources.color}>
				<FixtureColorDot color={fixture.color} />
				{fixture.colorLabel}
				{fixture.preloadColor && (
					<small className="preload-value">
						<FixtureColorDot color={fixture.preloadColor} /> Preload
					</small>
				)}
			</SourceValue>
		),
	};
}

function positionColumn(): Column {
	return {
		id: "position",
		header: "Position",
		width: "minmax(145px,1.25fr)",
		render: (fixture) => (
			<SourceValue source={fixture.sources.position}>
				<i className="position-glyph">
					<i
						style={{
							left: `${fixture.pan % 75}%`,
							top: `${fixture.tilt % 65}%`,
						}}
					/>
				</i>
				{fixture.positionLabel ?? `${fixture.pan}° / ${fixture.tilt}°`}
				{fixture.preloadPan != null && fixture.preloadTilt != null && (
					<small className="preload-value">
						→ {fixture.preloadPan} / {fixture.preloadTilt}
					</small>
				)}
			</SourceValue>
		),
	};
}

function valueColumn(id: "beam" | "focus", header: "Beam" | "Focus"): Column {
	return {
		id,
		header,
		width: "minmax(80px,.8fr)",
		render: (fixture) => (
			<SourceValue source={fixture.sources[id]}>{fixture[id]}</SourceValue>
		),
	};
}

export function fixtureSheetColumns(
	showType: boolean,
	present: FixtureStepPresenter,
): Column[] {
	return [
		fixtureIdColumn(present),
		fixtureIconColumn(),
		fixtureNameColumn(showType),
		patchColumn(),
		dimmerColumn(),
		colorColumn(),
		positionColumn(),
		valueColumn("beam", "Beam"),
		valueColumn("focus", "Focus"),
	];
}
