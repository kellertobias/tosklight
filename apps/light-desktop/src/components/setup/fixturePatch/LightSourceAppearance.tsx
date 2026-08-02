import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { useCallback, useState } from "react";
import type {
	InstalledFixtureAppearance,
	MultiPatchInstance,
	PatchedFixture,
} from "../../../api/types";
import { type PatchController, usePatchController } from "./controller";
import { fixtureDisplayId } from "./fixtureIds";
import { LightSourceAppearanceForm } from "./LightSourceAppearanceForm";
import {
	appearanceDraft,
	DEFAULT_APPEARANCE,
	gelSummary,
	normalizeAppearanceDraft,
	profileCct,
	sourceSummary,
	toPatchAppearance,
} from "./lightSourceAppearanceModel";

export function LightSourceCell({
	fixture,
	instance,
}: {
	fixture: PatchedFixture;
	instance?: MultiPatchInstance;
}) {
	const controller = usePatchController();
	const appearance = instance
		? instance.installed_appearance
		: fixture.installed_appearance;
	const current = appearance ?? DEFAULT_APPEARANCE;
	const available = hasGeometryEmitter(fixture);
	if (!available)
		return (
			<td className="patch-stacked-cell">
				<span className="patch-stacked-line">Unavailable</span>
				<span className="patch-stacked-line patch-secondary">
					No geometry emitter
				</span>
			</td>
		);

	const source = sourceSummary(fixture, current);
	const gel = gelSummary(current.gel);
	const target = instance?.name || fixtureDisplayId(fixture);
	return (
		<td className="patch-stacked-cell">
			<Button
				className="patch-value patch-stacked-editor"
				aria-label={`Light source ${target}: ${source}; ${gel}`}
				onClick={() =>
					beginLightSourceEdit(controller, fixture, instance?.id ?? null)
				}
			>
				<span className="patch-stacked-line" title={source}>
					{source}
				</span>
				<span className="patch-stacked-line patch-secondary" title={gel}>
					{gel}
				</span>
			</Button>
		</td>
	);
}

export function LightSourceAppearanceDialog() {
	const controller = usePatchController();
	const target = controller.ui.appearanceEdit;
	if (!target) return null;
	const fixture = controller.data.all.find(
		(candidate) => candidate.fixture_id === target.fixtureId,
	);
	const instance = target.multipatchInstanceId
		? fixture?.multipatch?.find(
				(candidate) => candidate.id === target.multipatchInstanceId,
			)
		: undefined;
	if (!fixture || (target.multipatchInstanceId && !instance)) return null;
	const identity = instance?.name || fixtureDisplayId(fixture);
	const appearance =
		(instance ? instance.installed_appearance : fixture.installed_appearance) ??
		DEFAULT_APPEARANCE;
	return (
		<AppearanceEditor
			key={`${fixture.fixture_id}:${target.multipatchInstanceId ?? "primary"}`}
			controller={controller}
			fixture={fixture}
			instance={instance}
			identity={identity}
			appearance={appearance}
		/>
	);
}

function AppearanceEditor({
	controller,
	fixture,
	instance,
	identity,
	appearance,
}: {
	controller: PatchController;
	fixture: PatchedFixture;
	instance?: MultiPatchInstance;
	identity: string | number;
	appearance: InstalledFixtureAppearance;
}) {
	const [draft, setDraft] = useState(() => appearanceDraft(appearance));
	const [submitError, setSubmitError] = useState("");
	const [catalogError, setCatalogError] = useState("");
	const [busy, setBusy] = useState(false);
	const result = normalizeAppearanceDraft(
		draft,
		appearance,
		profileCct(fixture),
	);
	const baseline = toPatchAppearance(appearance);
	const changed =
		result.appearance !== undefined &&
		JSON.stringify(result.appearance) !== JSON.stringify(baseline);
	const close = () => closeLightSourceEdit(controller);
	const onCatalogError = useCallback((message: string) => {
		setCatalogError(message);
	}, []);
	const apply = async () => {
		if (!result.appearance || !changed || busy) return;
		setBusy(true);
		setSubmitError("");
		const applied = await controller.patch.updateFixtureIntent(
			fixture.fixture_id,
			instance?.id ?? null,
			{ type: "set_installed_appearance", appearance: result.appearance },
		);
		setBusy(false);
		if (applied) close();
		else setSubmitError("The installed appearance could not be applied.");
	};
	return (
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer">
				<section
					className="nested-modal patch-edit-modal light-source-editor"
					role="dialog"
					aria-modal="true"
					aria-label={`Set light source ${identity}`}
				>
					<ModalTitleBar
						title={`Set light source ${identity}`}
						actions={
							<Button
								className="primary"
								disabled={!changed || Boolean(result.error) || busy}
								onClick={() => void apply()}
							>
								Apply
							</Button>
						}
						closeLabel="Close light source editor"
						onClose={close}
					/>
					<LightSourceAppearanceForm
						draft={draft}
						setDraft={setDraft}
						fixture={fixture}
						controller={controller}
						onCatalogError={onCatalogError}
					/>
					{(result.error || submitError || catalogError) && (
						<p className="patch-status" role="alert">
							{result.error || submitError || catalogError}
						</p>
					)}
				</section>
			</div>
		</ModalRegistration>
	);
}

function beginLightSourceEdit(
	controller: PatchController,
	fixture: PatchedFixture,
	multipatchInstanceId: string | null,
) {
	if (!controller.appState.patchSetArmed || !hasGeometryEmitter(fixture))
		return;
	controller.ui.setSelectedFixture(fixture.fixture_id);
	controller.patch.selectPatchInstance({
		fixtureId: fixture.fixture_id,
		multipatchInstanceId,
	});
	controller.ui.setAppearanceEdit({
		fixtureId: fixture.fixture_id,
		multipatchInstanceId,
	});
}

function closeLightSourceEdit(controller: PatchController) {
	controller.ui.setAppearanceEdit(null);
	controller.dispatch({ type: "SET_PATCH_ARMED", value: false });
}

export function hasGeometryEmitter(fixture: PatchedFixture) {
	const snapshot = fixture.definition.profile_snapshot;
	const modeId = fixture.definition.mode_id;
	return Boolean(
		snapshot &&
			modeId &&
			snapshot.modes.find((mode) => mode.id === modeId)?.geometry.emitters
				.length,
	);
}
