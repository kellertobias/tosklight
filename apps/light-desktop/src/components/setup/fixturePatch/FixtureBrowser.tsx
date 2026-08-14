import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { usePatchController } from "./controller";
import { FixtureDetails, FixtureModeSelect } from "./fixtureDisplay";
import { beginPlacement, chooseFamily } from "./placementDraft";

export function FixtureBrowser() {
	const controller = usePatchController();
	if (!controller.ui.browserOpen) return null;
	const close = () => controller.ui.setBrowserOpen(false);
	return (
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer">
				<section className="nested-modal fixture-browser-modal">
					<ModalTitleBar
						className="fixture-browser-header"
						title="Add fixture"
						search={{
							value: controller.ui.query,
							onSearch: controller.ui.setQuery,
							ariaLabel: "Search",
							placeholder: "Search manufacturer, fixture, mode, or type",
							settingsConfiguration: [
								{
									kind: "select",
									id: "type",
									label: "Fixture type",
									value: controller.ui.typeFilter,
									options: [
										{ value: "", label: "All" },
										...controller.data.types.map((type) => ({
											value: type,
											label: type,
										})),
									],
								},
							],
							onSettingChange: (_, value) =>
								controller.ui.setTypeFilter(String(value)),
							onClearSettings: () => controller.ui.setTypeFilter(""),
						}}
						closeLabel="Close Add fixture"
						onClose={close}
					/>
					<div className="fixture-picker-columns">
						<ManufacturerColumn />
						<FamilyColumn />
						<ModeColumn />
					</div>
				</section>
			</div>
		</ModalRegistration>
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
			<FixtureModeSelect
				modes={family.modes}
				value={fixtureDefinitionKey(definition)}
				onChange={controller.ui.setDefinitionKey}
			/>
			<FixtureDetails definition={definition} />
			<Button className="primary" onClick={() => beginPlacement(controller)}>
				Add fixture
			</Button>
		</section>
	);
}
