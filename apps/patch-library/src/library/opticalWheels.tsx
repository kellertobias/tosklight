import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui";
import type { Dispatch, SetStateAction } from "react";
import type {
	FixtureProfile,
	FixtureProfileGobo,
	FixtureProfilePrism,
} from "../fixtureProfile";
import { AssetField } from "./assets";

/** Optical artwork belongs to the profile; channels choose a wheel's slot. */
export function OpticalWheelsSection({
	draft,
	onChange,
}: {
	draft: FixtureProfile;
	onChange: Dispatch<SetStateAction<FixtureProfile>>;
}) {
	const updateGobo = (index: number, patch: Partial<FixtureProfileGobo>) =>
		onChange((current) => ({
			...current,
			gobos: (current.gobos ?? []).map((slot, i) =>
				i === index ? { ...slot, ...patch } : slot,
			),
		}));
	return (
		<section>
			<h3>Optical wheels</h3>
			<p className="field-hint">
				Wheel numbers match Gobo 1, Gobo 2, Prism 1 and Prism 2 controls. Slot
				zero is open. Each wheel and slot pair must be unique.
			</p>
			<h4>Gobo artwork</h4>
			{(draft.gobos ?? []).map((gobo, index) => (
				<div key={index}>
					<FormLayout columns={3} minColumnWidth={145}>
						<NumberField
							label="Gobo wheel"
							min={1}
							max={8}
							value={gobo.wheel ?? 1}
							onChange={(event) =>
								updateGobo(index, { wheel: Number(event.target.value) })
							}
						/>
						<NumberField
							label="Gobo slot"
							min={0}
							max={63}
							value={gobo.slot}
							onChange={(event) =>
								updateGobo(index, { slot: Number(event.target.value) })
							}
						/>
						<TextField
							label="Gobo name"
							value={gobo.name ?? ""}
							onChange={(event) =>
								updateGobo(index, { name: event.target.value })
							}
						/>
						<AssetField
							label="Gobo artwork"
							value={gobo.artwork_asset ?? null}
							extensions={["png", "jpg", "jpeg", "webp"]}
							preview="image"
							onChange={(artwork_asset) => updateGobo(index, { artwork_asset })}
						/>
						<Button
							onClick={() =>
								onChange((current) => ({
									...current,
									gobos: (current.gobos ?? []).filter((_, i) => i !== index),
								}))
							}
						>
							Remove gobo slot
						</Button>
					</FormLayout>
				</div>
			))}
			<Button
				onClick={() =>
					onChange((current) => ({
						...current,
						gobos: [
							...(current.gobos ?? []),
							{
								wheel: 1,
								slot:
									Math.max(
										0,
										...(current.gobos ?? [])
											.filter((g) => (g.wheel ?? 1) === 1)
											.map((g) => g.slot),
									) + 1,
							},
						],
					}))
				}
			>
				Add gobo slot
			</Button>
			<PrismRepresentation draft={draft} onChange={onChange} />
		</section>
	);
}

function PrismRepresentation({
	draft,
	onChange,
}: {
	draft: FixtureProfile;
	onChange: Dispatch<SetStateAction<FixtureProfile>>;
}) {
	const updatePrism = (index: number, patch: Partial<FixtureProfilePrism>) =>
		onChange((current) => ({
			...current,
			prisms: (current.prisms ?? []).map((slot, i) =>
				i === index ? { ...slot, ...patch } : slot,
			),
		}));
	return (
		<>
			<h4>Prism representation</h4>
			<p className="field-hint">
				Spread is the angle from the beam axis to the outermost copies. Leave
				this list empty to retain the generic prism appearance.
			</p>
			{(draft.prisms ?? []).map((prism, index) => (
				<div key={index}>
					<FormLayout columns={5} minColumnWidth={145}>
						<NumberField
							label="Prism wheel"
							min={1}
							max={8}
							value={prism.wheel ?? 1}
							onChange={(event) =>
								updatePrism(index, { wheel: Number(event.target.value) })
							}
						/>
						<NumberField
							label="Prism slot"
							min={1}
							max={63}
							value={prism.slot}
							onChange={(event) =>
								updatePrism(index, { slot: Number(event.target.value) })
							}
						/>
						<SelectField
							label="Prism arrangement"
							value={prism.representation}
							options={[
								{ value: "radial", label: "Radial" },
								{ value: "linear", label: "Linear" },
							]}
							onChange={(representation) =>
								updatePrism(index, { representation })
							}
						/>
						<NumberField
							label="Prism copies"
							min={2}
							max={16}
							value={prism.facets}
							onChange={(event) =>
								updatePrism(index, { facets: Number(event.target.value) })
							}
						/>
						<NumberField
							label="Prism spread (degrees)"
							allowDecimal
							min={0}
							max={45}
							value={prism.spread_degrees}
							onChange={(event) =>
								updatePrism(index, {
									spread_degrees: Number(event.target.value),
								})
							}
						/>
						<Button
							onClick={() =>
								onChange((current) => ({
									...current,
									prisms: (current.prisms ?? []).filter((_, i) => i !== index),
								}))
							}
						>
							Remove prism slot
						</Button>
					</FormLayout>
				</div>
			))}
			<Button
				onClick={() =>
					onChange((current) => ({
						...current,
						prisms: [
							...(current.prisms ?? []),
							{
								wheel: 1,
								slot:
									Math.max(
										0,
										...(current.prisms ?? [])
											.filter((p) => (p.wheel ?? 1) === 1)
											.map((p) => p.slot),
									) + 1,
								representation: "radial",
								facets: 3,
								spread_degrees: 4,
							},
						],
					}))
				}
			>
				Add prism slot
			</Button>
		</>
	);
}
