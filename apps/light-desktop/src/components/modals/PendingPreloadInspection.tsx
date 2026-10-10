import { Button, ModalPortal, ModalTitleBar } from "@tosklight/ui";
import { useState } from "react";
import type { AttributeValue } from "../../api/types/playback";
import { usePatchedFixturesView } from "../../features/patch/PatchState";
import type { ProgrammerPreloadLifecycleActions } from "../../features/programmerPreloadLifecycle/contracts";
import { useProgrammerPreloadLifecycleView } from "../../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { useProgrammerPreloadPlaybackQueueView } from "../../features/programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView";
import type { ProgrammerPreloadValuesProjection } from "../../features/programmerPreloadValues/contracts";
import { useProgrammerPreloadInspectionValuesView } from "../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import type { HydratedProgrammingDynamicSemanticValue } from "../../features/programmerValues/contracts";
import { usePortableGroups } from "../../features/showObjects/ShowObjectsState";
import { fixtureSheetTargets } from "../../windows/fixtureSheetTargets";

type PendingColorProgram = Extract<
	AttributeValue,
	{ kind: "color_program" }
>["value"];
type PendingColorIntent = Extract<
	PendingColorProgram,
	{ kind: "semantic" }
>["intent"];
type PendingColorComponent = NonNullable<
	PendingColorIntent["spreads"]
>[number]["component"];
type PendingNativeColorRecipe = Extract<
	PendingColorProgram,
	{ kind: "direct" }
>["recipe"];
type PendingProgrammingComponent = NonNullable<
	Extract<
		HydratedProgrammingDynamicSemanticValue,
		{ type: "programming_release" }
	>["component"]
>;

export function PendingPreloadInspection({
	onClose,
	onRecord,
}: {
	onClose(): void;
	onRecord(): void;
}) {
	const values = useProgrammerPreloadInspectionValuesView();
	const queue = useProgrammerPreloadPlaybackQueueView();
	const lifecycle = useProgrammerPreloadLifecycleView();
	const fixtures = usePatchedFixturesView();
	const groups = usePortableGroups();
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const ready = lifecycle.ready && !!values && !!queue;
	const rows = pendingRows(values);
	const targets = new Map(
		fixtures.flatMap((fixture) =>
			fixtureSheetTargets(fixture).map(
				(target) => [target.fixtureId, target] as const,
			),
		),
	);
	const fixtureLabel = (id: string) => {
		const target = targets.get(id);
		return target
			? `Fixture ${target.displayId} · ${target.name}`
			: `Fixture ${id}`;
	};
	const groupLabel = (id: string) => {
		const group = groups.find((g) => g.id === id);
		return `Group ${id}${group?.body.name ? ` · ${group.body.name}` : ""}`;
	};
	const remove = async (run: () => Promise<unknown>) => {
		setBusy(true);
		setError(null);
		try {
			if (!(await run()))
				setError(
					"The pending entry could not be removed. Inspect the refreshed entries and retry.",
				);
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setBusy(false);
		}
	};
	return (
		<ModalPortal onClose={onClose}>
			<div
				className="modal-backdrop"
				onPointerDown={(event) => {
					if (event.target === event.currentTarget) onClose();
				}}
			>
				<section className="modal-card preload-store-card">
					<ModalTitleBar title="Pending Preload" onClose={onClose} />
					<p>
						Inspect or remove pending work. Live output stays unchanged until
						Preload GO.
					</p>
					{!ready && (
						<p role="status">Loading authoritative pending Preload…</p>
					)}
					<div style={{ maxHeight: "55vh", overflowY: "auto" }}>
						<h3>Programmer changes</h3>
						{ready && rows.length === 0 && (
							<p>No pending programmer changes.</p>
						)}
						<ProgrammerRows
							rows={rows}
							disabled={!ready || busy || lifecycle.pending}
							fixtureLabel={fixtureLabel}
							groupLabel={groupLabel}
							onRemove={(row) =>
								void remove(() =>
									removePendingRow(row, values!.revision, lifecycle.actions!),
								)
							}
						/>
						<h3>Playback actions</h3>
						{ready && queue!.actions.length === 0 && (
							<p>No pending playback actions.</p>
						)}
						<ol>
							{queue?.actions.map((entry, index) => (
								<li key={`${queue.revision}:${index}`}>
									<span>
										{entry.surface === "virtual"
											? "Virtual Playback"
											: "Playback"}{" "}
										{entry.page == null
											? entry.playbackNumber
											: `${entry.page}.${entry.playbackNumber}`}{" "}
										· {entry.action.replaceAll("_", " ").toUpperCase()} ·{" "}
										{entry.surface}
									</span>{" "}
									<Button
										disabled={!ready || busy || lifecycle.pending}
										onClick={() =>
											void remove(() =>
												lifecycle.actions!.removePendingPlayback(
													index,
													queue.revision,
												),
											)
										}
									>
										Remove playback action {index + 1}
									</Button>
								</li>
							))}
						</ol>
					</div>
					{(error || lifecycle.error) && (
						<p role="alert">{error ?? lifecycle.error?.message}</p>
					)}
					<div className="modal-actions">
						<Button onClick={onClose}>Close</Button>
						<Button
							disabled={!ready || busy || lifecycle.pending}
							onClick={onRecord}
						>
							Record pending…
						</Button>
					</div>
				</section>
			</div>
		</ModalPortal>
	);
}

function pendingValueLabel(value: AttributeValue): string {
	const number = (n: number) => Number(n.toFixed(3)).toString();
	const scalar = (v: { kind: "value" | "spread"; value: number | number[] }) =>
		Array.isArray(v.value) ? v.value.map(number).join(" → ") : number(v.value);
	switch (value.kind) {
		case "normalized":
			return `${number(value.value * 100)}%`;
		case "spread":
			return value.value.map((v) => `${number(v * 100)}%`).join(" → ");
		case "discrete":
			return value.value;
		case "raw_dmx":
		case "raw_dmx_exact":
			return `DMX ${number(value.value)}`;
		case "color_xyz":
			return `XYZ ${number(value.value.x)}, ${number(value.value.y)}, ${number(value.value.z)}`;
		case "position":
			return value.value.kind === "angles"
				? `Pan ${scalar(value.value.pan_degrees)}° · Tilt ${scalar(value.value.tilt_degrees)}°`
				: `Target ${value.value.reference.kind === "origin" ? "Origin" : value.value.reference.point_id} · Offset ${value.value.offset_metres.map(scalar).join(", ")} m`;
		case "zoom":
			return `${scalar(value.value.opening_degrees)}° ${value.value.convention} opening`;
		case "color_program":
			return value.value.kind === "semantic"
				? semanticColorLabel(value.value.intent)
				: nativeColorLabel(value.value.recipe);
		case "group_family":
			return `Group template · ${pendingValueLabel(value.value.template)}${
				value.value.members
					? ` · Members ${Object.entries(value.value.members)
							.map(([id, member]) => `${id}: ${pendingValueLabel(member)}`)
							.join("; ")}`
					: ""
			}`;
	}
}

function dynamicValueSummary(
	value: HydratedProgrammingDynamicSemanticValue,
): string {
	switch (value.type) {
		case "release":
			return "Release fixture attribute";
		case "programming_release":
			return `Release ${value.component == null ? "physical family" : componentLabel(value.component)}`;
		case "dynamic_on":
			return `Dynamic ${value.dynamic.embedded_fallback.name} · Lane ${value.lane_id} · ON · Size ${Number((value.overrides.size * 100).toFixed(3))}% · Speed ${value.overrides.speed_multiplier.numerator}/${value.overrides.speed_multiplier.denominator} · Phase ${value.overrides.phase_offset_degrees}°`;
		case "dynamic_off":
			return `Dynamic ${value.instance_link} · OFF`;
		case "fix_at":
			return `Fix At ${Number((value.value * 100).toFixed(3))}%`;
		case "programming_fix_at":
			return `Fix At ${value.mask.address.component ? componentLabel(value.mask.address.component) : value.mask.address.representation.kind.replaceAll("_", " ")} · ${pendingValueLabel(value.mask.family)}`;
		case "static":
			return `Static · ${pendingValueLabel(value.value)}`;
	}
}

function timingLabel(value: {
	fade: boolean;
	fadeMillis: number | null;
	delayMillis: number | null;
}) {
	return `${value.fade ? ` · Fade ${value.fadeMillis == null ? "desk default" : `${value.fadeMillis} ms`}` : ""}${value.delayMillis == null ? "" : ` · Delay ${value.delayMillis} ms`}`;
}

function pendingRows(values: ProgrammerPreloadValuesProjection | null) {
	return [
		...(values?.fixtureValues ?? []).map((entry) => ({
			...entry,
			kind: "fixture" as const,
			id: entry.fixtureId,
		})),
		...(values?.groupValues ?? []).map((entry) => ({
			...entry,
			kind: "group" as const,
			id: entry.groupId,
		})),
		...(values?.dynamicValues ?? []).map((entry, index) => ({
			...entry,
			kind: "dynamic" as const,
			id: entry.fixtureId,
			index,
		})),
		...(values?.groupReleaseValues ?? []).map((entry, index) => ({
			...entry,
			kind: "group_release" as const,
			id: entry.groupId,
			index,
		})),
	].sort((a, b) => a.programmerOrder - b.programmerOrder);
}

type PendingRow = ReturnType<typeof pendingRows>[number];
function removePendingRow(
	row: PendingRow,
	revision: number,
	actions: ProgrammerPreloadLifecycleActions,
) {
	switch (row.kind) {
		case "dynamic":
			return actions.removePendingDynamic(row.index, revision);
		case "group_release":
			return actions.removePendingGroupRelease(row.index, revision);
		case "fixture":
			return actions.removePendingFixtureValue(row.id, row.attribute, revision);
		case "group":
			return actions.removePendingGroupValue(row.id, row.attribute, revision);
	}
}
function ProgrammerRows({
	rows,
	disabled,
	fixtureLabel,
	groupLabel,
	onRemove,
}: {
	rows: PendingRow[];
	disabled: boolean;
	fixtureLabel(id: string): string;
	groupLabel(id: string): string;
	onRemove(row: PendingRow): void;
}) {
	return (
		<ol>
			{rows.map((row) => (
				<li
					key={`${row.kind}:${row.programmerOrder}:${"index" in row ? row.index : row.id}:${row.attribute}`}
				>
					<span style={{ overflowWrap: "anywhere" }}>
						{row.kind === "fixture" || row.kind === "dynamic"
							? fixtureLabel(row.id)
							: groupLabel(row.id)}{" "}
						· {row.attribute} ·{" "}
						{row.kind === "group_release"
							? "Release group attribute"
							: row.kind === "dynamic"
								? pendingDynamicLabel(row.value)
								: pendingValueLabel(row.value)}
						{"fade" in row ? timingLabel(row) : ""}
					</span>{" "}
					<Button disabled={disabled} onClick={() => onRemove(row)}>
						Remove programmer change {row.programmerOrder}
					</Button>
				</li>
			))}
		</ol>
	);
}

function componentLabel(component: PendingProgrammingComponent): string {
	if (component.kind === "color")
		return `Color ${component.component.replaceAll("_", " ")}`;
	if (component.kind === "color_wheel")
		return `Color wheel ${component.component}`;
	if (component.kind === "native_color")
		return `Native color ${component.component.channel_id}/${component.component.function_id}`;
	return component.kind.replaceAll("_", " ");
}

function pendingDynamicLabel(
	value: HydratedProgrammingDynamicSemanticValue,
): string {
	const summary = dynamicValueSummary(value);
	if (!("timing" in value)) return summary;
	return `${summary}${value.timing.fade_millis == null ? "" : ` · Fade ${value.timing.fade_millis} ms`}${value.timing.delay_millis == null ? "" : ` · Delay ${value.timing.delay_millis} ms`}`;
}

function nativeColorLabel(recipe: PendingNativeColorRecipe): string {
	const channels = recipe.channels.map(
		(channel) =>
			`${channel.channel_id}/${channel.function_id}: DMX ${channel.raw}`,
	);
	const spreads = (recipe.spreads ?? []).map(
		(spread) =>
			`${spread.binding.channel_id}/${spread.binding.function_id}: Spread ${spread.points.join(" → ")}`,
	);
	return `Direct color · ${[...channels, ...spreads].join(" · ")}`;
}

function semanticColorLabel(intent: PendingColorIntent): string {
	const number = (n: number) => Number(n.toFixed(3)).toString();
	const parts = [
		`Virtual RGB ${intent.recipe.rgb.map((v) => `${number(v * 100)}%`).join(" / ")}`,
		`Amber ${number(intent.recipe.amber * 100)}%`,
		`Output ${number(intent.relative_output * 100)}%`,
		`White ${number(intent.white_blend * 100)}% (${number(intent.white_target.kelvin)} K, Duv ${number(intent.white_target.duv)})`,
		`UV ${number(intent.uv.amount * 100)}%`,
		`XYZ ${number(intent.base_xyz.x)}, ${number(intent.base_xyz.y)}, ${number(intent.base_xyz.z)}`,
		...(intent.spreads ?? []).map(
			(spread) =>
				`Spread Color ${spread.component.replaceAll("_", " ")} ${spread.points.map((point) => colorSpreadPoint(spread.component, point)).join(" → ")}`,
		),
	];
	if (intent.allocation !== "preserve_recipe")
		parts.push(`Allocation ${intent.allocation.replaceAll("_", " ")}`);
	for (const constraint of intent.wheel_constraints ?? []) {
		parts.push(
			`Pinned wheel ${constraint.value.channel_id}/${constraint.value.function_id}: DMX ${constraint.value.raw} · Source ${constraint.source.profile_id} revision ${constraint.source.profile_revision} mode ${constraint.source.mode_id} head ${constraint.source.head_id}`,
		);
	}
	return parts.join(" · ");
}

function colorSpreadPoint(
	component: PendingColorComponent,
	point: number,
): string {
	const number = (n: number) => Number(n.toFixed(3)).toString();
	switch (component) {
		case "hue":
			return `${number(point)}°`;
		case "temperature":
			return `${number(point)} K`;
		case "duv":
			return number(point);
		default:
			return `${number(point * 100)}%`;
	}
}
