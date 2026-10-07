import {
	defaultPositionCalibration,
	positionCalibrationError,
	positionCalibrationIsStale,
	type PositionCalibrationContext,
	type InstalledPositionCalibration,
} from "@tosklight/patch";
import {
	Button,
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui";
import { useState } from "react";
import { usePatchController } from "./controller";
import { fixtureDisplayId } from "./fixtureIds";

/** Opens calibration for the exact physical row whose Pan / Tilt editor is open. */
export function PositionCalibrationButton() {
	const controller = usePatchController();
	const copy = controller.ui.multipatchEdit;
	const fixtureId = copy?.fixtureId ?? controller.data.selected?.fixture_id;
	if (!fixtureId) return null;
	return (
		<Button
			onClick={() =>
				controller.ui.setPositionCalibrationEdit({
					fixtureId,
					multipatchInstanceId: copy?.instanceId ?? null,
				})
			}
		>
			Position calibration…
		</Button>
	);
}

export function PositionCalibrationDialog() {
	const controller = usePatchController();
	const target = controller.ui.positionCalibrationEdit;
	if (!target) return null;
	const fixture = controller.data.all.find(
		(value) => value.fixture_id === target.fixtureId,
	);
	const physical = target.multipatchInstanceId
		? fixture?.multipatch?.find(
				(value) => value.id === target.multipatchInstanceId,
			)
		: fixture;
	if (!fixture || !physical) return null;
	const identity = target.multipatchInstanceId
		? physical.name || "Multi-patch"
		: fixtureDisplayId(fixture);
	return (
		<PositionCalibrationEditor
			key={`${target.fixtureId}:${target.multipatchInstanceId ?? "root"}`}
			identity={String(identity)}
			initial={physical.position_calibration ?? null}
			context={fixture.definition.position_calibration_context}
			invertPan={physical.invert_pan ?? false}
			invertTilt={physical.invert_tilt ?? false}
			onClose={() => controller.ui.setPositionCalibrationEdit(null)}
			onSave={(calibration) =>
				controller.patch.updateFixtureIntent(
					target.fixtureId,
					target.multipatchInstanceId,
					{ type: "set_position_calibration", calibration },
				)
			}
		/>
	);
}

export function PositionCalibrationEditor({
	identity,
	initial,
	context,
	invertPan = false,
	invertTilt = false,
	onClose,
	onSave,
}: {
	context?: PositionCalibrationContext;
	invertPan?: boolean;
	invertTilt?: boolean;
	identity: string;
	initial: InstalledPositionCalibration | null;
	onClose(): void;
	onSave(value: InstalledPositionCalibration | null): Promise<boolean>;
}) {
	const [draft, setDraft] = useState<InstalledPositionCalibration | null>(() =>
		initial ? { ...initial } : null,
	);
	const [busy, setBusy] = useState(false);
	const [saveError, setSaveError] = useState("");
	const value = draft ?? defaultPositionCalibration();
	const error = positionCalibrationError(draft);
	const stale = positionCalibrationIsStale(draft?.axis_overrides, context);
	const changed = JSON.stringify(draft) !== JSON.stringify(initial);
	const update = (change: Partial<InstalledPositionCalibration>) => {
		setSaveError("");
		setDraft({ ...value, ...change });
	};
	const close = () => {
		if (!busy) onClose();
	};
	const save = async () => {
		if (busy || error || stale || !changed) return;
		setBusy(true);
		setSaveError("");
		try {
			if (await onSave(draft)) onClose();
			else
				setSaveError(
					"The position calibration could not be saved. Review the current fixture and try again.",
				);
		} catch (reason) {
			setSaveError(
				reason instanceof Error
					? reason.message
					: "The position calibration could not be saved.",
			);
		} finally {
			setBusy(false);
		}
	};
	return (
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer">
				<section
					className="nested-modal patch-edit-modal"
					role="dialog"
					aria-modal="true"
					aria-label={`Position calibration ${identity}`}
				>
					<ModalTitleBar
						title={`Position calibration ${identity}`}
						accept={{
							id: "save",
							label: "Save",
							variant: "primary",
							disabled: busy || Boolean(error) || stale || !changed,
							onPress: () => void save(),
						}}
						closeLabel="Close position calibration"
						onClose={close}
					/>
					<p className="field-hint">
						These offsets belong to this physical fixture only. They correct how
						Position Angles and Target values reach its DMX and how Stage aims it.
						Mounting rotation and current Pan / Tilt inversion remain separate.
					</p>
					<div className="form-grid">
						<NumberField
							label="Pan zero offset (°)"
							allowDecimal
							value={value.pan_zero_degrees}
							disabled={busy}
							onChange={(event) =>
								update({ pan_zero_degrees: Number(event.target.value) })
							}
						/>
						<NumberField
							label="Tilt zero offset (°)"
							allowDecimal
							value={value.tilt_zero_degrees}
							disabled={busy}
							onChange={(event) =>
								update({ tilt_zero_degrees: Number(event.target.value) })
							}
						/>
						<SelectField
							label="Calibration quality"
							ariaLabel="Calibration quality"
							value={value.quality}
							disabled={busy}
							options={[
								{ value: "unknown", label: "Unknown" },
								{ value: "estimated", label: "Estimated" },
								{ value: "manufacturer", label: "Manufacturer" },
								{ value: "measured", label: "Measured" },
							]}
							onChange={(quality) =>
								update({
									quality: quality as InstalledPositionCalibration["quality"],
								})
							}
						/>
						<TextField
							label="Calibration source"
							value={value.source ?? ""}
							disabled={busy}
							placeholder="Measurement or installation record"
							onChange={(event) =>
								update({ source: event.target.value || null })
							}
						/>
						<NumberField
							label="Calibration revision"
							min={0}
							max={0xffff_ffff}
							value={value.revision}
							disabled={busy}
							onChange={(event) =>
								update({ revision: Number(event.target.value) })
							}
						/>
					</div>
					<p className="field-hint">
						A positive zero offset adds degrees after the physical angle's
						direction correction. Values stay unwrapped across full turns.
					</p>

					{stale && (
						<p role="status">
							Axis overrides belong to a different profile, mode or geometry and
							are inactive. Clear the overrides before editing calibration for
							this lamp.
						</p>
					)}
					{!!context?.axes.length && (
						<fieldset disabled={busy || stale}>
							<legend>Individual physical axes</legend>
							<p className="field-hint">
								Each override replaces both the family zero and inversion. It is
								applied once; values remain unwrapped.
							</p>
							{context.axes.map((axis) => {
								const override = value.axis_overrides?.axes.find(
									(a) => a.node_id === axis.node_id,
								);
								const set = (change: {
									zero_degrees?: number;
									invert?: boolean;
								}) => {
									const entry = {
										node_id: axis.node_id,
										zero_degrees:
											axis.role === "pan"
												? value.pan_zero_degrees
												: value.tilt_zero_degrees,
										invert: axis.role === "pan" ? invertPan : invertTilt,
										...override,
										...change,
									};
									update({
										axis_overrides: {
											version: 1,
											source_identity: context.identity,
											axes: [
												...(value.axis_overrides?.axes.filter(
													(a) => a.node_id !== axis.node_id,
												) ?? []),
												entry,
											],
										},
									});
								};
								return (
									<fieldset key={axis.node_id}>
										<legend>{axis.name}</legend>
										{override ? (
											<>
												<NumberField
													label={`${axis.name} zero offset (°)`}
													allowDecimal
													value={override.zero_degrees}
													onChange={(e) =>
														set({ zero_degrees: Number(e.target.value) })
													}
												/>
												<SelectField
													label={`${axis.name} direction`}
													ariaLabel={`${axis.name} direction`}
													value={override.invert ? "inverted" : "normal"}
													options={[
														{ value: "normal", label: "Normal" },
														{ value: "inverted", label: "Inverted" },
													]}
													onChange={(v) => set({ invert: v === "inverted" })}
												/>
												<Button
													onClick={() => {
														const axes = value.axis_overrides!.axes.filter(
															(a) => a.node_id !== axis.node_id,
														);
														update({
															axis_overrides: axes.length
																? { ...value.axis_overrides!, axes }
																: null,
														});
													}}
												>
													Use {axis.role} defaults for {axis.name}
												</Button>
											</>
										) : (
											<Button onClick={() => set({})}>
												Override {axis.name}
											</Button>
										)}
									</fieldset>
								);
							})}
						</fieldset>
					)}
					{value.axis_overrides && (
						<Button
							disabled={busy}
							onClick={() => update({ axis_overrides: null })}
						>
							Clear axis overrides
						</Button>
					)}
					<Button
						disabled={busy || draft === null}
						onClick={() => setDraft(null)}
					>
						Clear calibration
					</Button>
					{(error || saveError) && (
						<p className="patch-status" role="alert">
							{error || saveError}
						</p>
					)}
				</section>
			</div>
		</ModalRegistration>
	);
}
