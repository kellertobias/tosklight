import { Button } from "@tosklight/ui";
import type { MultiPatchInstance, PatchedFixture } from "../../../api/types";
import { usePatchController } from "./controller";
import { armEdit } from "./editSession";
import { fixtureDisplayId } from "./fixtureIds";
import { beginMultipatchEdit } from "./multipatchActions";
import { fixturePolicyApplicability } from "./patchModel";

/** Multi-patch rows inherit fixture-level policy; repeating it adds no information. */
const INHERITED = "—";

export function FixtureModeCell({
	fixture,
	shared = false,
}: {
	fixture: PatchedFixture;
	shared?: boolean;
}) {
	const controller = usePatchController();
	const product = fixture.definition.name || fixture.definition.model;
	const mode = `${fixture.definition.manufacturer} · ${fixture.definition.mode}`;
	if (shared)
		return (
			<td className="patch-stacked-cell shared">
				<span>{INHERITED}</span>
			</td>
		);
	return (
		<td className="patch-stacked-cell">
			<Button
				className="patch-value patch-stacked-editor"
				aria-label={`Fixture and mode ${fixtureDisplayId(fixture)}: ${product} · ${fixture.definition.mode}`}
				title={`${product} · ${mode}`}
				onClick={() => armEdit(controller, fixture, "mode")}
			>
				<span className="patch-stacked-line">{product}</span>
				<span className="patch-stacked-line patch-stacked-detail">{mode}</span>
			</Button>
		</td>
	);
}

/** "-" when no master applies, otherwise which masters still control the fixture. */
function mastersSummary(fixture: PatchedFixture) {
	const applicable = fixturePolicyApplicability(fixture.definition);
	if (!applicable.groupMasters && !applicable.grandMaster) return "-";
	const group =
		applicable.groupMasters && (fixture.group_masters_enabled ?? true);
	const grand = applicable.grandMaster && (fixture.grand_master_enabled ?? true);
	if (group && grand) return "Both";
	if (group) return "Group";
	if (grand) return "Main";
	return "none";
}

/** "-" when the fixture has neither axis, otherwise the inverted axes. */
function panTiltSummary(
	fixture: PatchedFixture,
	instance?: MultiPatchInstance,
) {
	const applicable = fixturePolicyApplicability(fixture.definition);
	if (!applicable.pan && !applicable.tilt) return "-";
	const pan =
		applicable.pan && (instance?.invert_pan ?? fixture.invert_pan ?? false);
	const tilt =
		applicable.tilt && (instance?.invert_tilt ?? fixture.invert_tilt ?? false);
	if (pan && tilt) return "Invert Pan/Tilt";
	if (pan) return "Invert Pan";
	if (tilt) return "Invert Tilt";
	return "none";
}

export function MastersCell({
	fixture,
	shared = false,
}: {
	fixture: PatchedFixture;
	shared?: boolean;
}) {
	const controller = usePatchController();
	if (shared)
		return (
			<td className="patch-stacked-cell shared">
				<span>{INHERITED}</span>
			</td>
		);
	const summary = mastersSummary(fixture);
	if (summary === "-")
		return (
			<td>
				<span>{summary}</span>
			</td>
		);
	return (
		<td>
			<Button
				className="patch-value"
				aria-label={`Masters ${fixtureDisplayId(fixture)}`}
				onClick={() => armEdit(controller, fixture, "masters")}
			>
				{summary}
			</Button>
		</td>
	);
}

export function PanTiltCell({
	fixture,
	instance,
}: {
	fixture: PatchedFixture;
	instance?: MultiPatchInstance;
}) {
	const controller = usePatchController();
	const summary = panTiltSummary(fixture, instance);
	/** Only an instance that deviates from its fixture carries new information. */
	const inherited = Boolean(instance) && summary === panTiltSummary(fixture);
	if (summary === "-")
		return (
			<td>
				<span>{summary}</span>
			</td>
		);
	return (
		<td className={inherited ? "patch-secondary" : undefined}>
			<Button
				className="patch-value"
				aria-label={
					instance
						? `Pan and Tilt ${instance.name || "Multi-patch"}`
						: `Pan and Tilt ${fixtureDisplayId(fixture)}`
				}
				title={summary}
				onClick={() => {
					if (instance)
						beginMultipatchEdit(controller, fixture, instance, "pan_tilt");
					else armEdit(controller, fixture, "pan_tilt");
				}}
			>
				{inherited ? INHERITED : summary}
			</Button>
		</td>
	);
}

function formatMib(fixture: PatchedFixture) {
	if (!(fixture.move_in_black_enabled ?? true)) return "Off";
	const seconds = (fixture.move_in_black_delay_millis ?? 0) / 1_000;
	return `${Number(seconds.toFixed(3))} s`;
}

export function MibCell({
	fixture,
	shared = false,
}: {
	fixture: PatchedFixture;
	shared?: boolean;
}) {
	const controller = usePatchController();
	const value = formatMib(fixture);
	if (shared)
		return (
			<td className="patch-secondary">
				<span>{INHERITED}</span>
			</td>
		);
	return (
		<td>
			<Button
				className="patch-value"
				aria-label={`MIB ${fixtureDisplayId(fixture)}: ${value}`}
				onClick={() => armEdit(controller, fixture, "mib")}
			>
				{value}
			</Button>
		</td>
	);
}
