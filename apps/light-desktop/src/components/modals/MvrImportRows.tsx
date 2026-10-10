import { NumberField, SelectField } from "@tosklight/ui";
import type { QuickSetupModel } from "./QuickSetupModal";

type Fixture = NonNullable<
	QuickSetupModel["mvr"]["mvrPreview"]
>["fixtures"][number];
const addressStates: Record<string, string> = {
	import_unpatched: "Resolved · import unpatched",
	address: "Chosen address · validated on import",
	skip: "Resolved · skip fixture",
	replace: "Resolved · replace conflict",
};
export function MvrFixtureRow({
	fixture,
	model,
}: {
	fixture: Fixture;
	model: QuickSetupModel;
}) {
	const { mvrPreview, mvrResolutions, setMvrResolutions } = model.mvr;
	const resolution = mvrResolutions[fixture.uuid];
	const conflicted = mvrPreview?.address_conflicts.some((warning) =>
		warning.startsWith(fixture.name),
	);
	const action = resolution?.action ?? "import_unpatched";
	const update = (change: Record<string, string | number>) =>
		setMvrResolutions((current) => ({
			...current,
			[fixture.uuid]: {
				...current[fixture.uuid],
				action: "address",
				...change,
			},
		}));
	const state =
		conflicted && action === "skip"
			? addressStates.skip
			: !fixture.matched
				? "Imported unresolved · no DMX output"
				: !conflicted
					? fixture.universe && fixture.address
						? "Address retained"
						: "Imported unpatched"
					: (addressStates[action] ?? "Resolution selected");
	return (
		<article
			className="import-mapping-row"
			aria-label={`Import fixture ${fixture.name}`}
		>
			<div className="import-mapping-row__source">
				<b>{fixture.name}</b>
				<small>
					{fixture.gdtf_spec} · {fixture.gdtf_mode}
				</small>
				<small>
					{fixture.universe && fixture.address
						? `Source U${fixture.universe}.${fixture.address}`
						: "Source unpatched"}
				</small>
			</div>
			<div className="import-mapping-row__target">
				{conflicted ? (
					<>
						<SelectField
							label={`Resolution for ${fixture.name}`}
							value={action}
							options={[
								{ value: "import_unpatched", label: "Import unpatched" },
								{ value: "address", label: "Choose address" },
								{ value: "skip", label: "Skip" },
								{ value: "replace", label: "Replace conflict" },
							]}
							onChange={(action) =>
								setMvrResolutions((current) => ({
									...current,
									[fixture.uuid]: {
										action,
										universe: fixture.universe ?? 1,
										address: fixture.address ?? 1,
									},
								}))
							}
						/>
						{resolution?.action === "address" && (
							<div className="mvr-address-fields">
								<NumberField
									label="Universe"
									min={1}
									max={65535}
									aria-label={`Universe for ${fixture.name}`}
									value={resolution.universe ?? 1}
									onChange={(event) =>
										update({ universe: Number(event.target.value) })
									}
								/>
								<NumberField
									label="Address"
									min={1}
									max={512}
									aria-label={`Address for ${fixture.name}`}
									value={resolution.address ?? 1}
									onChange={(event) =>
										update({ address: Number(event.target.value) })
									}
								/>
							</div>
						)}
					</>
				) : (
					<span>
						{fixture.universe && fixture.address
							? `Destination U${fixture.universe}.${fixture.address}`
							: "Destination unpatched"}
					</span>
				)}
			</div>
			<div className="import-mapping-row__state" role="status">
				{state}
			</div>
		</article>
	);
}
