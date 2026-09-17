import {
	FormLayout,
	ModalRegistration,
	ModalTitleBar,
	MultiValueToggleField,
	SwitchField,
} from "@tosklight/ui";
import { useEffect, useState } from "react";
import type { RecordUpdateOption } from "../../api/types";
import {
	optionLabel,
	RECORD_UPDATE_OPTIONS,
} from "../../features/recordUpdateOptions/options";

export interface RecordUpdateChoiceModalProps {
	kind: "record" | "update";
	/** The stored default; `null` while it is still loading. */
	storedDefault: RecordUpdateOption | null;
	/** A one-off option the armed command line already names. */
	initialOption?: RecordUpdateOption | null;
	busy: boolean;
	error: string | null;
	onConfirm: (option: RecordUpdateOption, setAsDefault: boolean) => void;
	onCancel: () => void;
	/** Update only: opens the list of everything the programmer can update. */
	onTargets?: () => void;
}

/** RECORD RECORD / UPDATE UPDATE: choose how this Record or Update stores the programmer. */
export function RecordUpdateChoiceModal({
	kind,
	storedDefault,
	initialOption = null,
	busy,
	error,
	onConfirm,
	onCancel,
	onTargets,
}: RecordUpdateChoiceModalProps) {
	const verb = kind === "record" ? "Record" : "Update";
	const [option, setOption] = useState<RecordUpdateOption>(
		initialOption ?? storedDefault ?? "smart",
	);
	const [setAsDefault, setSetAsDefault] = useState(false);
	useEffect(() => {
		if (storedDefault) setOption(initialOption ?? storedDefault);
	}, [initialOption, storedDefault]);
	const selected = RECORD_UPDATE_OPTIONS.find(
		(candidate) => candidate.value === option,
	);
	const loading = storedDefault === null;
	return (
		<ModalRegistration onClose={onCancel}>
			<div
				className={`modal-backdrop ${kind === "update" ? "update-workflow-layer" : ""}`.trim()}
				onPointerDown={(event) =>
					event.target === event.currentTarget && onCancel()
				}
			>
				<section
					className={`modal-card record-update-choice-modal workflow-theme ${
						kind === "record"
							? "store-settings-modal record-workflow"
							: "update-settings-modal update-workflow"
					}`}
					role="dialog"
					aria-modal="true"
					aria-label={verb}
				>
					<ModalTitleBar
						title={verb}
						groups={[
							{
								id: `${kind}-choice-actions`,
								actions: [
									...(onTargets
										? [
												{
													id: "targets",
													label: "Targets",
													disabled: busy,
													onPress: onTargets,
												},
											]
										: []),
									{
										id: "cancel",
										label: "Cancel",
										disabled: busy,
										onPress: onCancel,
									},
								],
							},
						]}
						accept={{
							id: kind,
							label: verb,
							variant: "primary",
							disabled: busy || loading,
							onPress: () => onConfirm(option, setAsDefault),
						}}
						closeLabel={`Close ${verb}`}
						closeDisabled={busy}
						onClose={onCancel}
					/>
					<FormLayout labelPlacement="top">
						<MultiValueToggleField
							label="Mode"
							ariaLabel={`${verb} mode`}
							value={option}
							disabled={busy || loading}
							onChange={setOption}
							options={RECORD_UPDATE_OPTIONS.map(({ value, label }) => ({
								value,
								label,
							}))}
							description={
								kind === "record" ? selected?.record : selected?.update
							}
						/>
						<SwitchField
							label="Set as default"
							offLabel={`This ${verb} only`}
							onLabel="Default"
							checked={setAsDefault}
							disabled={busy || loading}
							onChange={(event) => setSetAsDefault(event.target.checked)}
							description={
								<span className="record-update-current-default">
									Current default:{" "}
									<b>{storedDefault ? optionLabel(storedDefault) : "…"}</b>
									{storedDefault && storedDefault !== "smart"
										? " · Smart with Set as default returns to the regular behaviour."
										: ""}
								</span>
							}
						/>
					</FormLayout>
					{error && (
						<p className="modal-error" role="alert">
							{error}
						</p>
					)}
				</section>
			</div>
		</ModalRegistration>
	);
}
