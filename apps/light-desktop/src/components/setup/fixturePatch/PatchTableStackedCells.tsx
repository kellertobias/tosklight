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
	accessibleName,
	onClick,
}: {
	label: string;
	state: string;
	available: boolean;
	shared: boolean;
	accessibleName: string;
	onClick: () => void;
}) {
	const content = (
		<>
			<span>{label}</span>
			<strong>{available ? state : "Unavailable"}</strong>
			{shared && available && <small>Shared</small>}
		</>
	);
	if (shared || !available)
		return (
			<span
				className="patch-stacked-line"
				title={`${label} ${available ? state : "unavailable"}`}
			>
				{content}
			</span>
		);
	return (
		<Button
			className="patch-value patch-stacked-line"
			aria-label={accessibleName}
			onClick={onClick}
		>
			{content}
		</Button>
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
	return (
		<td className={`patch-stacked-cell${shared ? " shared" : ""}`}>
			<PolicyLine
				label="Group Masters"
				state={
					(fixture.group_masters_enabled ?? true)
						? "Controlled"
						: "Not controlled"
				}
				available={applicable.groupMasters}
				shared={shared}
				accessibleName={`Group Masters ${fixtureDisplayId(fixture)}`}
				onClick={() => armEdit(controller, fixture, "group_masters")}
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
				accessibleName={`Grand Master ${fixtureDisplayId(fixture)}`}
				onClick={() => armEdit(controller, fixture, "grand_master")}
			/>
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
	const edit = (axis: "pan" | "tilt") => {
		if (instance)
			beginMultipatchEdit(
				controller,
				fixture,
				instance,
				axis === "pan" ? "invert_pan" : "invert_tilt",
			);
		else
			armEdit(
				controller,
				fixture,
				axis === "pan" ? "invert_pan" : "invert_tilt",
			);
	};
	return (
		<td className="patch-stacked-cell">
			<PolicyLine
				label="Invert Pan"
				state={
					(instance?.invert_pan ?? fixture.invert_pan ?? false)
						? "Inverted"
						: "Normal"
				}
				available={applicable.pan}
				shared={false}
				accessibleName={
					instance
						? `Invert pan ${instance.name || "Multi-patch"}`
						: `Invert Pan ${fixtureDisplayId(fixture)}`
				}
				onClick={() => edit("pan")}
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
				accessibleName={
					instance
						? `Invert tilt ${instance.name || "Multi-patch"}`
						: `Invert Tilt ${fixtureDisplayId(fixture)}`
				}
				onClick={() => edit("tilt")}
			/>
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

function kelvin(value: number | null | undefined) {
	return value == null
		? null
		: `${Math.round(value).toLocaleString("en-US")} K`;
}

export function LightSourceCell({
	fixture,
	shared = false,
}: {
	fixture: PatchedFixture;
	shared?: boolean;
}) {
	const temperature = kelvin(
		fixture.definition.profile_snapshot?.physical.color_temperature_kelvin,
	);
	const source = `Profile default${temperature ? ` · ${temperature}` : ""}`;
	return (
		<td className={`patch-stacked-cell${shared ? " shared" : ""}`}>
			<span className="patch-stacked-line" title={source}>
				{source}
			</span>
			<span className="patch-stacked-line patch-secondary">Open white</span>
		</td>
	);
}
