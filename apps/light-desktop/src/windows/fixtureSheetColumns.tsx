import type { DataTableColumn } from "@tosklight/ui/window-kit";
import { FixtureColorDot } from "../components/shared/FixtureColorDot";
import { SourceValue } from "../components/shared/SourceValue";
import type { FixtureSheetCompactMode } from "../types";
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

function fixtureNameColumn(
	showType: boolean,
	compactMode: FixtureSheetCompactMode,
): Column {
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
					<GroupMasterStatus fixture={fixture} compact={compactMode !== "off"} />
				)}
			</span>
		),
	};
}

function GroupMasterStatus({
	fixture,
	compact,
}: {
	fixture: FixtureSheetRow;
	compact: boolean;
}) {
	const flash = fixture.limitingGroups.some(
		(group) => group.runtime.flashLevel > 0,
	);
	const state = fixture.highlightBypassesGroupMaster
		? "highlight-bypass"
		: flash
			? "flash"
			: "limited";
	const effective = limitingGroupPercentage(fixture);
	const label = fixture.highlightBypassesGroupMaster
		? compact
			? "GM bypass · Highlight"
			: "Group master bypassed · Highlight"
		: flash
			? `${compact ? "GM" : "Group master"} ${effective}% · Flash`
			: `${compact ? "GM" : "Group master"} ${effective}%`;
	return (
		<em
			className="fixture-group-master-status"
			data-group-master-state={state}
			title={groupMasterTitle(fixture)}
		>
			◒ {label}
		</em>
	);
}

function limitingGroupPercentage(fixture: FixtureSheetRow) {
	return Math.round(
		Math.max(...fixture.limitingGroups.map(effectiveGroupMaster)) * 100,
	);
}

function effectiveGroupMaster(group: FixtureSheetRow["limitingGroups"][number]) {
	return Math.max(group.runtime.master, group.runtime.flashLevel);
}

function groupMasterTitle(fixture: FixtureSheetRow) {
	const groups = fixture.limitingGroups
		.map((group) => {
			const master = Math.round(group.runtime.master * 100);
			const flash = Math.round(group.runtime.flashLevel * 100);
			return flash > 0
				? `${group.body.name}: fader ${master}%, Flash ${flash}%, effective ${Math.round(effectiveGroupMaster(group) * 100)}%`
				: `${group.body.name}: ${master}%`;
		})
		.join(", ");
	return fixture.highlightBypassesGroupMaster
		? `${groups}; bypassed by Highlight`
		: groups;
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

function dimmerColumn(compactMode: FixtureSheetCompactMode): Column {
	return {
		id: "intensity",
		header: "Intensity",
		width:
			compactMode === "off" ? "minmax(180px,.9fr)" : "minmax(128px,.7fr)",
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
					<span
						className="fixture-sheet-group-presentation"
						role="img"
						aria-label={group?.accessibleName ?? `Intensity ${fixture.dimmer}%`}
					>
						<i className="vertical-meter">
							<i style={{ height: `${fixture.dimmer}%` }} />
						</i>
						<span className="fixture-sheet-value-text">
							{member?.text ?? `${fixture.dimmer}%`}
						</span>
					</span>
					{(member?.preloadText ??
						(fixture.preloadDimmer == null
							? null
							: `${fixture.preloadDimmer}%`)) && (
						<small className="preload-value">
							<span className="fixture-sheet-preload-marker">→</span>{" "}
							<span className="fixture-sheet-value-text">
								{member?.preloadText ?? `${fixture.preloadDimmer}%`}
							</span>
						</small>
					)}
					<DynamicIndicators member={member} />
				</SourceValue>
			);
		},
	};
}

function colorColumn(compactMode: FixtureSheetCompactMode): Column {
	return {
		id: "color",
		header: "Color",
		width: compactMode === "off" ? "minmax(105px,1fr)" : "minmax(72px,.65fr)",
		render: (fixture) => (
			<SourceValue
				source={fixture.groupValues?.color.source ?? fixture.sources.color}
				className="fixture-sheet-group-value"
			>
				<span
					className="fixture-sheet-group-presentation"
					role="img"
					aria-label={
						fixture.groupValues?.color.accessibleName ?? fixture.colorLabel
					}
				>
					<FixtureColorDot color={fixture.color} />
					<span className="fixture-sheet-value-text">{fixture.colorLabel}</span>
				</span>
				{fixture.preloadColor && (
					<small className="preload-value">
						<FixtureColorDot color={fixture.preloadColor} />
						<span className="fixture-sheet-preload-marker">→</span>{" "}
						<span className="fixture-sheet-value-text">Preload</span>
					</small>
				)}
				<GroupDynamicIndicators group={fixture.groupValues?.color} />
			</SourceValue>
		),
	};
}

function positionColumn(compactMode: FixtureSheetCompactMode): Column {
	return {
		id: "position",
		header: "Position",
		width:
			compactMode === "off" ? "minmax(145px,1.25fr)" : "minmax(86px,.75fr)",
		render: (fixture) => (
			<SourceValue
				source={
					fixture.groupValues?.position.source ?? fixture.sources.position
				}
				className="fixture-sheet-group-value"
			>
				<span
					className="fixture-sheet-group-presentation"
					role="img"
					aria-label={
						fixture.groupValues?.position.accessibleName ??
						fixture.positionLabel ??
						`${fixture.pan}° / ${fixture.tilt}°`
					}
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
				</span>
				{fixture.preloadPan != null && fixture.preloadTilt != null && (
					<small className="preload-value">
						<span className="fixture-sheet-preload-marker">→</span>{" "}
						<span className="fixture-sheet-value-text">
							{fixture.preloadPan} / {fixture.preloadTilt}
						</span>
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
	compactMode: FixtureSheetCompactMode,
): Column {
	return {
		id,
		header,
		width:
			compactMode === "off"
				? id === "media"
					? "minmax(280px,1.6fr)"
					: "minmax(95px,.8fr)"
				: id === "media"
					? "minmax(160px,1fr)"
					: "minmax(64px,.6fr)",
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
					<span
						className="fixture-sheet-group-presentation"
						role="img"
						aria-label={group.accessibleName}
					>
						{group.available ? (
							group.members.map((member) => (
								<span
									className="fixture-sheet-member-value"
									key={member.attribute}
								>
									<MemberGlyph member={member} group={id} />
									<span className="fixture-sheet-value-text">
										{group.members.length > 1 ? `${member.label} ` : ""}
										{member.text}
									</span>
									{member.preloadText && (
										<small className="preload-value">
											<MemberGlyph
												member={member}
												group={id}
												preload
												value={member.preloadValue ?? member.value}
											/>
											<span className="fixture-sheet-preload-marker">→</span>{" "}
											<span className="fixture-sheet-value-text">
												{member.preloadText}
											</span>
										</small>
									)}
									<DynamicIndicators member={member} />
								</span>
							))
						) : (
							<span title={group.accessibleName}>—</span>
						)}
					</span>
				</SourceValue>
			);
		},
	};
}

function MemberGlyph({
	member,
	group,
	preload = false,
	value = member.value,
}: {
	member: FixtureSheetMemberValue;
	group: "beam" | "shapers" | "focus" | "control" | "media";
	preload?: boolean;
	value?: FixtureSheetMemberValue["value"];
}) {
	const normalized = value.kind === "normalized" ? value.value : null;
	const semantic =
		group === "media"
			? member.label.startsWith("Mask")
				? member.label.endsWith("Folder")
					? "MaF"
					: "Ma"
				: member.label.endsWith("Folder")
					? "MeF"
					: "Me"
			: (member.text.match(/[A-Za-z0-9]/)?.[0]?.toUpperCase() ?? "·");
	return (
		<i
			className={`fixture-sheet-group-glyph${preload ? " preload" : ""}`}
			data-group={group}
			data-semantic-value={member.text}
			style={
				normalized == null
					? undefined
					: ({ "--fixture-value": normalized } as React.CSSProperties)
			}
			aria-hidden="true"
		>
			{semantic}
		</i>
	);
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
	compactMode: FixtureSheetCompactMode = "off",
): Column[] {
	return [
		fixtureIdColumn(present),
		fixtureIconColumn(),
		fixtureNameColumn(showType, compactMode),
		patchColumn(),
		dimmerColumn(compactMode),
		colorColumn(compactMode),
		positionColumn(compactMode),
		valueColumn("beam", "Beam", compactMode),
		valueColumn("shapers", "Shapers", compactMode),
		valueColumn("focus", "Focus", compactMode),
		valueColumn("control", "Control", compactMode),
		valueColumn("media", "Media", compactMode),
	];
}
