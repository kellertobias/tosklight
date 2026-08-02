import { Button } from "@tosklight/ui";
import type { MultiPatchInstance, PatchedFixture } from "../../../api/types";
import { usePatchController } from "./controller";
import { armEdit } from "./editSession";
import { fixtureDisplayId } from "./fixtureIds";
import { beginMultipatchEdit } from "./multipatchActions";
import { fixturePolicyApplicability } from "./patchModel";

export function FixtureModeCell({
	fixture,
	shared = false,
}: {
	fixture: PatchedFixture;
	shared?: boolean;
}) {
	const controller = usePatchController();
	const productMode = `${fixture.definition.name || fixture.definition.model} · ${fixture.definition.mode}`;
	const manufacturer = fixture.definition.manufacturer;
	return (
		<td className={`patch-stacked-cell${shared ? " shared" : ""}`}>
			{shared ? (
				<span className="patch-stacked-line" title={productMode}>
					{productMode}
				</span>
			) : (
				<Button
					className="patch-value patch-stacked-line"
					aria-label={`Fixture and mode ${fixtureDisplayId(fixture)}: ${productMode}`}
					title={productMode}
					onClick={() => armEdit(controller, fixture, "mode")}
				>
					{productMode}
				</Button>
			)}
			<span className="patch-stacked-line patch-secondary" title={manufacturer}>
				{manufacturer}
			</span>
		</td>
	);
}

function PolicyLine({
	label,
	state,
	available,
	shared,
}: {
	label: string;
	state: string;
	available: boolean;
	shared: boolean;
}) {
	const content = (
		<>
			<span>{label}</span>
			<strong>{available ? state : "Unavailable"}</strong>
			{shared && available && <small>Shared</small>}
		</>
	);
	return (
		<span
			className="patch-stacked-line"
			title={`${label} ${available ? state : "unavailable"}${shared ? ", shared" : ""}`}
		>
			{content}
		</span>
	);
}

export function MastersCell({
	fixture,
	shared = false,
}: {
	fixture: PatchedFixture;
	shared?: boolean;
}) {
	const controller = usePatchController();
	const applicable = fixturePolicyApplicability(fixture.definition);
	const content = (
		<>
			<PolicyLine
				label="Group Masters"
				state={
					(fixture.group_masters_enabled ?? true)
						? "Controlled"
						: "Not controlled"
				}
				available={applicable.groupMasters}
				shared={shared}
			/>
			<PolicyLine
				label="Grand Master"
				state={
					(fixture.grand_master_enabled ?? true)
						? "Controlled"
						: "Not controlled"
				}
				available={applicable.grandMaster}
				shared={shared}
			/>
		</>
	);
	return (
		<td className={`patch-stacked-cell${shared ? " shared" : ""}`}>
			{shared || (!applicable.groupMasters && !applicable.grandMaster) ? (
				<span>{content}</span>
			) : (
				<Button
					className="patch-value patch-stacked-editor"
					aria-label={`Masters ${fixtureDisplayId(fixture)}`}
					onClick={() => armEdit(controller, fixture, "masters")}
				>
					{content}
				</Button>
			)}
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
	const applicable = fixturePolicyApplicability(fixture.definition);
	const edit = () => {
		if (instance)
			beginMultipatchEdit(controller, fixture, instance, "pan_tilt");
		else armEdit(controller, fixture, "pan_tilt");
	};
	const content = (
		<>
			<PolicyLine
				label="Invert Pan"
				state={
					(instance?.invert_pan ?? fixture.invert_pan ?? false)
						? "Inverted"
						: "Normal"
				}
				available={applicable.pan}
				shared={false}
			/>
			<PolicyLine
				label="Invert Tilt"
				state={
					(instance?.invert_tilt ?? fixture.invert_tilt ?? false)
						? "Inverted"
						: "Normal"
				}
				available={applicable.tilt}
				shared={false}
			/>
		</>
	);
	return (
		<td className="patch-stacked-cell">
			{!applicable.pan && !applicable.tilt ? (
				<span>{content}</span>
			) : (
				<Button
					className="patch-value patch-stacked-editor"
					aria-label={
						instance
							? `Pan and Tilt ${instance.name || "Multi-patch"}`
							: `Pan and Tilt ${fixtureDisplayId(fixture)}`
					}
					onClick={edit}
				>
					{content}
				</Button>
			)}
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
	return (
		<td className={shared ? "patch-shared-cell" : undefined}>
			{shared ? (
				<span title={`MIB ${value}, shared`}>
					{value}
					<small>Shared</small>
				</span>
			) : (
				<Button
					className="patch-value"
					aria-label={`MIB ${fixtureDisplayId(fixture)}: ${value}`}
					onClick={() => armEdit(controller, fixture, "mib")}
				>
					{value}
				</Button>
			)}
		</td>
	);
}
