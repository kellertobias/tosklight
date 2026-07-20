import { useEffect, useMemo, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useCueRecording } from "../../features/cueRecording/CueRecordingProvider";
import {
	useCueLists,
	usePresets,
} from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useApp } from "../../state/AppContext";
import {
	Button,
	FormLayout,
	ModalPortal,
	NumberField,
	SelectField,
	TextField,
} from "../common";

type StoreTarget = "preset" | "cue";
type PresetRecordMode = "merge" | "overwrite" | "add_missing_fixtures";

export function PreloadStoreModal() {
	const { state, dispatch } = useApp();
	const server = useServer();
	const cueRecording = useCueRecording();
	const presets = usePresets();
	const cueLists = useCueLists();
	const [target, setTarget] = useState<StoreTarget>("preset");
	const [presetId, setPresetId] = useState("1");
	const [cueListId, setCueListId] = useState("");
	const [cueNumber, setCueNumber] = useState(1);
	const [name, setName] = useState("");
	const [mode, setMode] = useState<PresetRecordMode>("merge");
	const open = state.preloadStoreOpen;
	const targetId = target === "preset" ? presetId : cueListId;

	useShowObjectView("preset", open && target === "preset");
	useShowObjectView("cue_list", open && target === "cue");
	useDefaultCueList(target, cueListId, cueLists, setCueListId);

	const targetObject = useMemo(
		() =>
			target === "preset"
				? presets.find((object) => object.id === targetId)
				: cueLists.find((object) => object.id === targetId),
		[target, targetId, presets, cueLists],
	);
	if (!open) return null;

	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "preloadStoreOpen", value: false });
	const submit = async () => {
		const stored =
			target === "cue"
				? await recordCue(cueRecording, targetId, cueNumber, name)
				: await server.storePreload(
						{
							target,
							target_id: targetId,
							name: name || undefined,
							mode,
						},
						targetObject?.revision ?? 0,
					);
		if (stored) close();
	};

	return (
		<ModalPortal>
			<div
				className="modal-backdrop"
				onPointerDown={(event) => {
					if (event.target === event.currentTarget) close();
				}}
			>
				<section className="modal-card preload-store-card">
					<Button className="modal-close" onClick={close}>
						×
					</Button>
					<h2>Record Pending Preload</h2>
					<p>The active preload scene remains live. Only the pending scene will be stored.</p>
					<TargetSelector target={target} onChange={setTarget} />
					<FormLayout className="preload-target-form" labelPlacement="side">
						{target === "preset" ? (
							<TextField
								label="Preset slot"
								value={presetId}
								onChange={(event) => setPresetId(event.target.value)}
							/>
						) : (
							<SelectField
								label="Cuelist"
								value={cueListId}
								onChange={setCueListId}
								options={cueLists.map((cueList) => ({
									value: cueList.id,
									label: cueList.body.name || cueList.id,
								}))}
							/>
						)}
						{target === "cue" && (
							<NumberField
								label="Cue number"
								allowDecimal
								step="0.1"
								value={cueNumber}
								onChange={(event) => setCueNumber(Number(event.target.value))}
							/>
						)}
						{target === "preset" && (
							<SelectField
								label="Record mode"
								value={mode}
								onChange={setMode}
								options={presetRecordModes}
							/>
						)}
						<TextField
							label="Name"
							clearable
							value={name}
							onChange={(event) => setName(event.target.value)}
							placeholder="Optional name"
						/>
					</FormLayout>
					<div className="modal-actions">
						<Button onClick={close}>Cancel</Button>
						<Button disabled={!targetId} onClick={() => void submit()}>
							Record to {target === "preset" ? `Preset ${targetId}` : `Cue ${cueNumber}`}
						</Button>
					</div>
					{targetObject && (
						<small>
							Existing target revision {targetObject.revision}; normal conflict protection applies.
						</small>
					)}
					{server.error && <p className="modal-error">{server.error}</p>}
				</section>
			</div>
		</ModalPortal>
	);
}

function useDefaultCueList(
	target: StoreTarget,
	cueListId: string,
	cueLists: ReturnType<typeof useCueLists>,
	setCueListId: (id: string) => void,
) {
	useEffect(() => {
		if (target !== "cue" || cueLists.some((cueList) => cueList.id === cueListId))
			return;
		setCueListId(cueLists[0]?.id ?? "");
	}, [cueListId, cueLists, setCueListId, target]);
}

async function recordCue(
	actions: ReturnType<typeof useCueRecording>,
	cueListId: string,
	cueNumber: number,
	name: string,
) {
	if (!actions) return false;
	return Boolean(
		await actions.record({
			target: { kind: "cue_list", cueListId },
			operation: "overwrite",
			cueNumber,
			timing: {},
			cueOnly: false,
			name: name || undefined,
			capturePolicy: "pending_or_active_preload",
			activationPolicy: "hold",
		}),
	);
}

function TargetSelector(props: {
	target: StoreTarget;
	onChange(target: StoreTarget): void;
}) {
	return (
		<div className="segmented-control">
			<Button
				className={props.target === "preset" ? "active" : ""}
				onClick={() => props.onChange("preset")}
			>
				Preset
			</Button>
			<Button
				className={props.target === "cue" ? "active" : ""}
				onClick={() => props.onChange("cue")}
			>
				Cue
			</Button>
		</div>
	);
}

const presetRecordModes: Array<{ value: PresetRecordMode; label: string }> = [
	{ value: "merge", label: "Merge" },
	{ value: "overwrite", label: "Overwrite" },
	{ value: "add_missing_fixtures", label: "Add missing fixtures" },
];
