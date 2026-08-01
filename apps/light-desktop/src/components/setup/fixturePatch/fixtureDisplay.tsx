import { SelectField } from "@tosklight/ui";
import type { FixtureDefinition } from "../../../api/types";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { fixtureTypeIconAsset } from "../fixtureTypeIconAssets";
import { isDmxPatchable } from "../patchUtils";

export function FixtureTypeIcon({ type }: { type: string }) {
	return (
		<span
			className="fixture-type-icon"
			title={type || "other"}
			role="img"
			aria-label={`Type: ${type || "other"}`}
		>
			<img alt="" src={fixtureTypeIconAsset(type)} />
		</span>
	);
}

export function MultiPatchBranch({ last }: { last: boolean }) {
	return (
		<span className="multipatch-branch" aria-hidden="true">
			<svg viewBox="0 0 28 42" aria-hidden="true">
				<path d={last ? "M7 0v20q0 6 6 6h12" : "M7 0v42M7 20q0 6 6 6h12"} />
			</svg>
		</span>
	);
}

export function FixtureModeSelect({
	modes,
	value,
	onChange,
}: {
	modes: FixtureDefinition[];
	value: string;
	onChange: (value: string) => void;
}) {
	return (
		<SelectField
			label="Mode"
			ariaLabel="Mode"
			value={value}
			onChange={onChange}
			options={modes.map((mode) => ({
				value: fixtureDefinitionKey(mode),
				label: (
					<span className="fixture-mode-option-label">
						<span>{mode.mode}</span>
						{isDmxPatchable(mode) ? (
							<span>{mode.footprint}ch</span>
						) : (
							<small className="fixture-mode-no-dmx">No DMX</small>
						)}
					</span>
				),
			}))}
		/>
	);
}

export function FixtureDetails({
	definition,
}: {
	definition: FixtureDefinition;
}) {
	return (
		<div className="fixture-details">
			<strong>
				{isDmxPatchable(definition)
					? `${definition.footprint} DMX channels`
					: "Visual only · no DMX patch"}
			</strong>
			<span>{definition.device_type}</span>
			<span>
				{definition.heads.length} head{definition.heads.length === 1 ? "" : "s"}
			</span>
			<span>Revision {definition.revision}</span>
			{definition.physical.width_millimetres && (
				<span>
					{definition.physical.width_millimetres} ×{" "}
					{definition.physical.height_millimetres ?? "?"} ×{" "}
					{definition.physical.depth_millimetres ?? "?"} mm
				</span>
			)}
		</div>
	);
}
