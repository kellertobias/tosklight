import { Button, ModalTitleBar, Select } from "../../common";
import { SearchBar } from "../../common/SearchBar";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { isDmxPatchable } from "../patchUtils";
import { usePatchController } from "./controller";
import { FixtureDetails } from "./fixtureDisplay";
import { beginPlacement, chooseFamily } from "./placementDraft";

export function FixtureBrowser() {
	const controller = usePatchController();
	if (!controller.ui.browserOpen) return null;
	return (
		<div className="stacked-modal-layer">
			<section className="nested-modal fixture-browser-modal">
				<ModalTitleBar
					title="Add fixture"
					search={<FixtureBrowserSearch />}
					closeLabel="Close Add fixture"
					onClose={() => controller.ui.setBrowserOpen(false)}
				/>
				<div className="fixture-picker-columns">
					<ManufacturerColumn />
					<FamilyColumn />
					<ModeColumn />
				</div>
			</section>
		</div>
	);
}

function FixtureBrowserSearch() {
	const { ui, data } = usePatchController();
	return (
		<SearchBar
			value={ui.query}
			onChange={ui.setQuery}
			filters={[{ id: "type", label: "Fixture type", options: data.types }]}
			values={{ type: ui.typeFilter }}
			onFilterChange={(_, value) => ui.setTypeFilter(value)}
			placeholder="Search manufacturer, fixture, mode, or type"
		/>
	);
}

function ManufacturerColumn() {
	const { ui, data } = usePatchController();
	return (
		<section>
			<h3>Manufacturer</h3>
			<Button
				className={!ui.manufacturer ? "active" : ""}
				onClick={() => ui.setManufacturer("")}
			>
				<span>All manufacturers</span>
			</Button>
			{data.manufacturers.map((name) => (
				<Button
					className={ui.manufacturer === name ? "active" : ""}
					key={name}
					onClick={() => ui.setManufacturer(name)}
				>
					<span>{name}</span>
				</Button>
			))}
		</section>
	);
}

function FamilyColumn() {
	const controller = usePatchController();
	return (
		<section>
			<h3>Fixture</h3>
			{controller.data.families.map((item) => (
				<Button
					className={controller.data.family?.key === item.key ? "active" : ""}
					key={item.key}
					onClick={() => chooseFamily(controller, item.key)}
				>
					<span>{item.name}</span>
					<small>
						{item.deviceType} · {item.modes.length} modes
					</small>
				</Button>
			))}
		</section>
	);
}

function ModeColumn() {
	const controller = usePatchController();
	const { family, definition } = controller.data;
	if (!family || !definition)
		return (
			<section className="fixture-mode-detail">
				<p>Select a fixture.</p>
			</section>
		);
	return (
		<section className="fixture-mode-detail">
			<h3>
				{family.manufacturer} {family.name}
			</h3>
			{/* biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label. */}
			<label>
				Mode
				<Select
					aria-label="Mode"
					value={fixtureDefinitionKey(definition)}
					onChange={(event) =>
						controller.ui.setDefinitionKey(event.target.value)
					}
				>
					{family.modes.map((mode) => (
						<option
							value={fixtureDefinitionKey(mode)}
							key={fixtureDefinitionKey(mode)}
						>
							{mode.mode} ·{" "}
							{isDmxPatchable(mode) ? `${mode.footprint}ch` : "No DMX"}
						</option>
					))}
				</Select>
			</label>
			<FixtureDetails definition={definition} />
			<Button className="primary" onClick={() => beginPlacement(controller)}>
				Add fixture
			</Button>
		</section>
	);
}
