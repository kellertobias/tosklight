import type { DataTableColumn } from "@tosklight/ui/window-kit";
import { FixtureColorDot } from "../components/shared/FixtureColorDot";
import { SourceValue } from "../components/shared/SourceValue";
import type { FixtureSheetRow } from "./fixtureSheetProjection";
import type { FixtureStepPresenter } from "./fixtureSheetStep";
import type {
	FixtureSheetAttributeGroup,
	FixtureSheetDynamicIdentity,
	FixtureSheetGroupValue,
	FixtureSheetMemberValue,
} from "./fixtureSheetValues";

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
				`${group.body.name}: ${Math.round(group.runtime.master * 100)}%`,
		)
		.join(", ");
}

function limitingGroupPercentage(fixture: FixtureSheetRow) {
	return Math.round(
		Math.max(...fixture.limitingGroups.map((group) => group.runtime.master)) *
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
		header: "Intensity",
		width: "minmax(95px,.7fr)",
		render: (fixture) => {
			const group = fixture.groupValues?.intensity;
			const member = group?.members.find(
				(candidate) => candidate.attribute === "intensity",
			);
			return (
				<SourceValue
					source={group?.source ?? fixture.sources.dimmer}
					className="fixture-sheet-group-value"
				>
					<i className="vertical-meter">
						<i style={{ height: `${fixture.dimmer}%` }} />
					</i>
					<span className="fixture-sheet-value-text">
						{member?.text ?? `${fixture.dimmer}%`}
					</span>
					{(member?.preloadText ??
						(fixture.preloadDimmer == null
							? null
							: `${fixture.preloadDimmer}%`)) && (
						<small className="preload-value">
							→ {member?.preloadText ?? `${fixture.preloadDimmer}%`}
						</small>
					)}
					<DynamicIndicators member={member} />
				</SourceValue>
			);
		},
	};
}

function colorColumn(): Column {
	return {
		id: "color",
		header: "Color",
		width: "minmax(105px,1fr)",
		render: (fixture) => (
			<SourceValue
				source={fixture.groupValues?.color.source ?? fixture.sources.color}
				className="fixture-sheet-group-value"
			>
				<FixtureColorDot color={fixture.color} />
				<span className="fixture-sheet-value-text">{fixture.colorLabel}</span>
				{fixture.preloadColor && (
					<small className="preload-value">
						<FixtureColorDot color={fixture.preloadColor} /> Preload
					</small>
				)}
				<GroupDynamicIndicators group={fixture.groupValues?.color} />
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
			<SourceValue
				source={
					fixture.groupValues?.position.source ?? fixture.sources.position
				}
				className="fixture-sheet-group-value"
			>
				<i className="position-glyph">
					<i
						style={{
							left: `${fixture.pan % 75}%`,
							top: `${fixture.tilt % 65}%`,
						}}
					/>
				</i>
				<span className="fixture-sheet-value-text">
					{fixture.groupValues?.position.members
						.map((member) => member.text)
						.join(" / ") ??
						fixture.positionLabel ??
						`${fixture.pan}° / ${fixture.tilt}°`}
				</span>
				{fixture.preloadPan != null && fixture.preloadTilt != null && (
					<small className="preload-value">
						→ {fixture.preloadPan} / {fixture.preloadTilt}
					</small>
				)}
				<GroupDynamicIndicators group={fixture.groupValues?.position} />
			</SourceValue>
		),
	};
}

function DynamicIndicators({ member }: { member?: FixtureSheetMemberValue }) {
	if (!member?.dynamics.length) return null;
	return (
		<span className="fixture-dynamic-indicators">
			{member.dynamics.map((dynamic, index) => (
				<DynamicIndicator
					key={`${dynamic.lane}:${dynamic.dynamicId ?? dynamic.label}:${index}`}
					dynamic={dynamic}
				/>
			))}
		</span>
	);
}

function DynamicIndicator({
	dynamic,
}: {
	dynamic: FixtureSheetDynamicIdentity;
}) {
	return (
		<small
			className="fixture-dynamic-stack"
			role="img"
			data-hidden={dynamic.hidden || undefined}
			data-lane={dynamic.lane}
			data-paused={dynamic.paused || undefined}
			data-pending={dynamic.pending || undefined}
			data-winning={dynamic.winning || undefined}
			title={dynamic.accessibleName}
			aria-label={dynamic.accessibleName}
		>
			<span aria-hidden="true">∿</span> {dynamic.label}
		</small>
	);
}

function GroupDynamicIndicators({ group }: { group?: FixtureSheetGroupValue }) {
	if (!group) return null;
	return (
		<>
			{group.members.map((member) => (
				<DynamicIndicators key={member.attribute} member={member} />
			))}
		</>
	);
}

function valueColumn(
	id: "beam" | "shapers" | "focus" | "control" | "media",
	header: "Beam" | "Shapers" | "Focus" | "Control" | "Media",
): Column {
	return {
		id,
		header,
		width: id === "media" ? "minmax(180px,1.2fr)" : "minmax(95px,.8fr)",
		render: (fixture) => {
			const group = fixture.groupValues?.[id as FixtureSheetAttributeGroup];
			if (!group)
				return (
					<SourceValue source={legacySource(fixture, id)}>
						{id === "beam" || id === "focus" ? fixture[id] : "—"}
					</SourceValue>
				);
			return (
				<SourceValue
					source={group.source}
					className="fixture-sheet-group-value"
				>
					{group.available ? (
						group.members.map((member) => (
							<span
								className="fixture-sheet-member-value"
								key={member.attribute}
							>
								<span className="fixture-sheet-value-text">
									{group.members.length > 1 ? `${member.label} ` : ""}
									{member.text}
								</span>
								{member.preloadText && (
									<small className="preload-value">
										→ {member.preloadText}
									</small>
								)}
								<DynamicIndicators member={member} />
							</span>
						))
					) : (
						<span title={group.accessibleName}>—</span>
					)}
				</SourceValue>
			);
		},
	};
}

function legacySource(
	fixture: FixtureSheetRow,
	id: "beam" | "shapers" | "focus" | "control" | "media",
) {
	return id === "beam" || id === "focus" ? fixture.sources[id] : "default";
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
		valueColumn("shapers", "Shapers"),
		valueColumn("focus", "Focus"),
		valueColumn("control", "Control"),
		valueColumn("media", "Media"),
	];
}
