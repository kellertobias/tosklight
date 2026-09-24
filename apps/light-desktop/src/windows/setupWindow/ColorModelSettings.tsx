import { Button, FormLayout, ModalPortal, ModalTitleBar, SelectField } from "@tosklight/ui";
import { useState } from "react";
import type {
	ColorModelImpact,
	ColorProgrammingModel,
} from "../../api/client/attributeConfiguration";
import type { SetupWindowController } from "./controller";

const MODEL_OPTIONS: ReadonlyArray<{
	value: ColorProgrammingModel;
	label: string;
}> = [
	{ value: "direct", label: "Direct — program fixture-native colour channels" },
	{ value: "intent", label: "Color Intent — program one colour for every fixture" },
];

export function modelLabel(model: ColorProgrammingModel) {
	return model === "intent" ? "Color Intent" : "Direct";
}

/** The active show's colour programming model, switched only through an explicit confirmation. */
export function ShowColorModelSettings({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const snapshot = controller.attributeConfiguration;
	const actions = controller.attributeActions;
	const [pending, setPending] = useState<ColorModelImpact | null>(null);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	if (!snapshot) return null;
	const current = snapshot.configuration.color_model ?? "direct";
	const apply = async (model: ColorProgrammingModel, acknowledge: boolean) => {
		if (!actions) return;
		setBusy(true);
		setError(null);
		try {
			const saved = await actions.update(
				snapshot,
				{ color_model: model },
				{ acknowledgeColorModelImpact: acknowledge },
			);
			await controller.adoptAttributeConfiguration(saved);
			setPending(null);
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setBusy(false);
		}
	};
	const choose = async (model: ColorProgrammingModel) => {
		if (!actions || model === current) return;
		setBusy(true);
		setError(null);
		try {
			const impact = await actions.colorModelImpact(model);
			setBusy(false);
			// Anything the switch changes about stored colour is shown before it happens.
			if (impact.items.length > 0) setPending(impact);
			else await apply(model, false);
		} catch (reason) {
			setBusy(false);
			setError(reason instanceof Error ? reason.message : String(reason));
		}
	};
	return (
		<article className="color-model-settings">
			<header>
				<b>Color programming model</b>
				<small>
					Belongs to this show. Color Intent programs one device-independent
					colour that every fixture reproduces as closely as it can; Intensity
					still sets the level. Direct programs each fixture's own colour
					channels.
				</small>
			</header>
			<FormLayout labelPlacement="side">
				<SelectField
					label="This show programs colour as"
					ariaLabel="Show color programming model"
					value={current}
					disabled={!actions?.canWrite || busy}
					options={MODEL_OPTIONS.map((option) => ({ ...option }))}
					onChange={(value) => void choose(value as ColorProgrammingModel)}
				/>
			</FormLayout>
			{busy && (
				<p className="color-model-progress" role="status">
					Checking the show’s stored colour…
				</p>
			)}
			{error && (
				<p className="modal-error" role="alert">
					{error}
				</p>
			)}
			{pending && (
				<ColorModelImpactDialog
					impact={pending}
					busy={busy}
					onCancel={() => setPending(null)}
					onConfirm={() => void apply(pending.to, pending.lossy)}
				/>
			)}
		</article>
	);
}

function ColorModelImpactDialog({
	impact,
	busy,
	onCancel,
	onConfirm,
}: {
	impact: ColorModelImpact;
	busy: boolean;
	onCancel(): void;
	onConfirm(): void;
}) {
	const title = `Switch to ${modelLabel(impact.to)}`;
	return (
		<ModalPortal onClose={onCancel}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onCancel()
				}
			>
				<section
					className="nested-modal color-model-impact-dialog"
					role="alertdialog"
					aria-modal="true"
					aria-label={title}
				>
					<ModalTitleBar
						title={title}
						closeLabel="Keep the current colour model"
						onClose={onCancel}
					/>
					<div className="color-model-impact">
						<b>
							{impact.lossy
								? "Some stored colour will not come back unchanged if you switch back."
								: "Stored colour is kept; this is how it behaves afterwards."}
						</b>
						<ul>
							{impact.items.map((item) => (
								<li
									key={item.kind}
									data-lossy={item.lossy ? "true" : undefined}
								>
									{item.message}
								</li>
							))}
						</ul>
						<small>The switch itself rewrites no stored value.</small>
						<div className="color-model-impact-actions">
							<Button autoFocus onClick={onCancel} disabled={busy}>
								Keep {modelLabel(impact.from)}
							</Button>
							<Button
								className={impact.lossy ? "danger" : undefined}
								onClick={onConfirm}
								disabled={busy}
							>
								Switch to {modelLabel(impact.to)}
							</Button>
						</div>
					</div>
				</section>
			</div>
		</ModalPortal>
	);
}

/** The desk's default for shows it creates. Existing shows keep the model they store. */
export function NewShowDefaultsSettings({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	if (!draft) return null;
	return (
		<article>
			<header>
				<b>New show defaults</b>
				<small>
					Copied into each show this desk creates. Changing it never changes a
					show that already exists.
				</small>
			</header>
			<FormLayout labelPlacement="side">
				<SelectField
					label="Color programming model"
					ariaLabel="Default color programming model for new shows"
					value={draft.color_programming_model_default ?? "direct"}
					options={MODEL_OPTIONS.map((option) => ({ ...option }))}
					onChange={(value) =>
						controller.editDraft({
							...draft,
							color_programming_model_default: value as ColorProgrammingModel,
						})
					}
				/>
			</FormLayout>
		</article>
	);
}
