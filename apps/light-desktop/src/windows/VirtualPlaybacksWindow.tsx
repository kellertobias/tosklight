import {
	Button,
	FormLayout,
	ModalRegistration,
	ModalTitleBar,
	TextField,
} from "@tosklight/ui";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import {
	virtualPlaybackBankStart,
	virtualPlaybackNumber,
} from "../api/virtualPlaybackAddress";
import { VirtualPlaybackConfigurationModal } from "../components/control/VirtualPlaybackConfigurationModal";
import { useVirtualPlaybackController } from "../components/control/virtualPlayback/useVirtualPlaybackController";
import { VirtualPlaybackGrid } from "../components/control/virtualPlayback/VirtualPlaybackGrid";
import { useCommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import { loadRecordSettings } from "../components/setup/ProgrammerDefaults";
import { usePaneChromeTargets } from "../components/shell/PaneChromeContext";
import { useControlSurfaceTarget } from "../features/controlSurfaceInteraction/useControlSurfaceTarget";
import { useCueRecording } from "../features/cueRecording/CueRecordingProvider";
import type { WindowProps } from "./windowTypes";

export function VirtualPlaybacksWindow({ paneId, active = true }: WindowProps) {
	const controller = useVirtualPlaybackController(paneId, active);
	const cueRecording = useCueRecording();
	const command = useCommandLineSurface({
		enabled: active,
		observeCommand: true,
	});
	const offPending = /^OFF$/iu.test((command.text ?? "").trim());
	const [recordChoice, setRecordChoice] = useState<{
		slot: number;
		cueNumber: string;
	} | null>(null);
	const paneChrome = usePaneChromeTargets();
	const recordVirtual = async (
		slot: number,
		choice: "add" | "merge" | "overwrite",
	) => {
		if (controller.pageNumber == null) return;
		const playbackNumber = virtualPlaybackNumber(controller.pageNumber, slot);
		const playback =
			controller.page?.virtual_playbacks?.[String(playbackNumber)] ?? null;
		const cueList =
			playback?.target.type === "cue_list"
				? controller.cueLists.get(playback.target.cue_list_id)
				: undefined;
		const cueNumber = choice === "add" ? undefined : cueList?.cues[0]?.number;
		const settings = loadRecordSettings();
		const outcome = await cueRecording?.record({
			target: {
				kind: "virtual",
				page: controller.pageNumber,
				playbackNumber,
			},
			operation: choice === "merge" ? "merge" : "overwrite",
			...(cueNumber ? { cueNumber } : {}),
			timing: {},
			cueOnly: settings.cueOnly,
			capturePolicy: "current_capture",
			activationPolicy: "hold",
		});
		if (!outcome) return;
		controller.dispatch({ type: "SET_STORE_ARMED", value: false });
		await command.reset();
	};
	const turnOffVirtual = async (slot: number) => {
		if (controller.pageNumber == null || !offPending) return;
		const playbackNumber = virtualPlaybackNumber(controller.pageNumber, slot);
		const playback =
			controller.page?.virtual_playbacks?.[String(playbackNumber)] ?? null;
		if (!playback) return;
		const outcome = await controller.runtimeActions?.virtualPlaybackAction(
			controller.pageNumber,
			playback.number,
			"off",
			{ surface: "virtual" },
		);
		if (outcome) await command.reset();
	};
	const requestVirtualRecord = (slot: number) => {
		if (controller.pageNumber == null) return;
		const playbackNumber = virtualPlaybackNumber(controller.pageNumber, slot);
		const playback =
			controller.page?.virtual_playbacks?.[String(playbackNumber)] ?? null;
		const cueList =
			playback?.target.type === "cue_list"
				? controller.cueLists.get(playback.target.cue_list_id)
				: undefined;
		if (cueList?.cues.length === 1) {
			setRecordChoice({ slot, cueNumber: cueList.cues[0].number });
			return;
		}
		void recordVirtual(slot, "add");
	};
	useControlSurfaceTarget({
		id: `virtual-playback-settings:${paneId ?? "builtin"}`,
		priority: 100,
		accepts: (intent) =>
			intent.type === "open_playback_settings" &&
			intent.playback.addressing === "virtual" &&
			intent.playback.pageNumber === controller.pageNumber &&
			intent.playback.pageObjectId === (controller.pageObject?.id ?? null) &&
			intent.playback.pageObjectRevision ===
				(controller.pageObject?.revision ?? 0),
		handle: (intent) => {
			if (
				intent.type !== "open_playback_settings" ||
				intent.playback.addressing !== "virtual" ||
				controller.pageNumber == null
			)
				return;
			const playbackNumber = intent.playback.playbackNumber;
			controller.openConfiguration(
				controller.page?.virtual_playbacks?.[String(playbackNumber)] ?? null,
				playbackNumber - virtualPlaybackBankStart(controller.pageNumber) + 1,
			);
		},
	});
	useEffect(() => {
		const openRequested = (event: Event) => {
			const detail = (
				event as CustomEvent<{
					addressing: string;
					page?: number | null;
					playback?: number | null;
				}>
			).detail;
			if (
				detail.addressing !== "virtual" ||
				detail.page == null ||
				detail.page !== controller.pageNumber ||
				detail.playback == null
			)
				return;
			controller.openConfiguration(
				controller.page?.virtual_playbacks?.[String(detail.playback)] ?? null,
				detail.playback - virtualPlaybackBankStart(detail.page) + 1,
			);
		};
		window.addEventListener("light:playback-configuration", openRequested);
		return () =>
			window.removeEventListener("light:playback-configuration", openRequested);
	}, [controller]);
	if (!controller.authorityReady || controller.pageNumber == null)
		return (
			<section className="virtual-playback-pane" aria-busy="true">
				<p
					role={
						controller.topology.error || controller.runtimeStatus.error
							? "alert"
							: "status"
					}
				>
					{controller.topology.error?.message ??
						controller.runtimeStatus.error?.message ??
						"Loading Virtual Playbacks…"}
				</p>
			</section>
		);
	return (
		<section
			className="virtual-playback-pane"
			aria-label={`Virtual Playbacks page ${controller.pageNumber}`}
		>
			{paneChrome?.toolbar &&
				createPortal(
					<VirtualPlaybackTitleActions
						zonesReady={controller.zones.ready}
						saving={controller.zones.saving}
						zoneCount={controller.zones.zones.length}
						selectedSlots={controller.selectedSlots}
						selectedPlaybackCount={controller.selectedPlaybackCount}
						editing={controller.zoneEdit !== null}
						onCreateZone={(name) => {
							controller.setZoneName(name);
							controller.setCreatingZone(true);
						}}
						onUpdateZone={() => void controller.updateZone()}
						onCancelZone={controller.cancelZoneSelection}
					/>,
					paneChrome.toolbar,
				)}
			<VirtualPlaybackGrid
				pageNumber={controller.pageNumber}
				page={controller.page}
				pageObjectId={controller.pageObject?.id ?? null}
				pageObjectRevision={controller.pageObject?.revision ?? 0}
				rows={controller.rows}
				columns={controller.columns}
				playbacks={controller.playbacks}
				cueLists={controller.cueLists}
				runtimes={controller.runtimes}
				runtimeActions={controller.runtimeActions}
				zones={controller.zones.zones}
				selectedSlots={controller.selectedSlots}
				configurationArmed={controller.configurationArmed}
				storeArmed={controller.state.storeArmed}
				updateArmed={controller.state.updateArmed}
				offPending={offPending}
				shiftArmed={controller.state.shiftArmed}
				onConfigure={controller.openConfiguration}
				onRecord={requestVirtualRecord}
				onOff={(slot) => void turnOffVirtual(slot)}
				onToggleZone={controller.toggleZoneSlot}
				paneId={paneId}
			/>
			{recordChoice && (
				<VirtualCueRecordChoiceModal
					cueNumber={recordChoice.cueNumber}
					onClose={() => setRecordChoice(null)}
					onChoice={(choice) => {
						const slot = recordChoice.slot;
						setRecordChoice(null);
						void recordVirtual(slot, choice);
					}}
				/>
			)}
			{controller.zones.error && (
				<p className="virtual-playback-pane-error" role="alert">
					{controller.zones.error}
				</p>
			)}
			{controller.topologyActionError && (
				<p className="virtual-playback-pane-error" role="alert">
					{controller.topologyActionError}
				</p>
			)}
			{controller.configuration && (
				<VirtualPlaybackConfigurationModal
					playback={controller.configuration.playback}
					page={controller.pageNumber}
					slot={controller.configuration.slot}
					empty={controller.configuration.empty}
					expectedPageRevision={controller.configuration.expectedPageRevision}
					expectedPageObjectId={controller.configuration.expectedPageObjectId}
					expectedPlaybackRevision={
						controller.configuration.expectedPlaybackRevision
					}
					expectedPlaybackObjectId={
						controller.configuration.expectedPlaybackObjectId
					}
					onClose={() => controller.setConfiguration(null)}
				/>
			)}
			{controller.creatingZone && (
				<CreateZoneModal
					playbackNumbers={controller.selectedSlots.map((slot) =>
						virtualPlaybackNumber(controller.pageNumber ?? 1, slot),
					)}
					name={controller.zoneName}
					error={controller.zones.error}
					saving={controller.zones.saving}
					onClose={() => controller.setCreatingZone(false)}
					onNameChange={controller.setZoneName}
					onCreate={(name) => void controller.createZone(name)}
				/>
			)}
		</section>
	);
}

function VirtualCueRecordChoiceModal(props: {
	cueNumber: string;
	onClose(): void;
	onChoice(choice: "add" | "merge" | "overwrite"): void;
}) {
	return createPortal(
		<ModalRegistration onClose={props.onClose}>
			<div className="stacked-modal-layer cue-record-choice-layer">
				<section
					className="nested-modal cue-record-choice-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Record Cue choice"
				>
					<ModalTitleBar title={`Record Cue ${props.cueNumber}`} />
					<p>This Cuelist contains one Cue. Choose how to record it.</p>
					<div className="command-choice-actions">
						<Button variant="primary" onClick={() => props.onChoice("add")}>
							Add Cue
						</Button>
						<Button onClick={() => props.onChoice("merge")}>Merge Cue</Button>
						<Button onClick={() => props.onChoice("overwrite")}>
							Overwrite Cue
						</Button>
						<Button onClick={props.onClose}>Cancel</Button>
					</div>
				</section>
			</div>
		</ModalRegistration>,
		document.body,
	);
}

export function VirtualPlaybackTitleActions(props: {
	zonesReady: boolean;
	saving: boolean;
	zoneCount: number;
	selectedSlots: readonly number[];
	selectedPlaybackCount?: number;
	editing: boolean;
	onCreateZone(name: string): void;
	onUpdateZone(): void;
	onCancelZone(): void;
}) {
	return (
		<span className="virtual-playback-title-actions">
			{props.editing ? (
				<>
					<Button
						className="primary"
						disabled={
							props.saving ||
							!props.zonesReady ||
							(props.selectedPlaybackCount ?? props.selectedSlots.length) < 2
						}
						onClick={props.onUpdateZone}
					>
						Update Exclusion Zone
					</Button>
					<Button onClick={props.onCancelZone}>Cancel Edit</Button>
				</>
			) : (
				props.zonesReady &&
				props.selectedSlots.length >= 2 && (
					<Button
						className="primary"
						onClick={() =>
							props.onCreateZone(`Exclusion Zone ${props.zoneCount + 1}`)
						}
					>
						Create Exclusion Zone
					</Button>
				)
			)}
			{!props.editing && props.selectedSlots.length > 0 && (
				<Button onClick={props.onCancelZone}>Cancel Zone Selection</Button>
			)}
		</span>
	);
}

function CreateZoneModal(props: {
	playbackNumbers: readonly number[];
	name: string;
	error: string | null;
	saving: boolean;
	onClose(): void;
	onNameChange(name: string): void;
	onCreate(name: string): void;
}) {
	const [draftName, setDraftName] = useState(props.name);
	return (
		<ModalRegistration onClose={props.onClose}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && props.onClose()
				}
			>
				<section
					className="nested-modal virtual-playback-zone-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Create Exclusion Zone"
				>
					<ModalTitleBar
						title="Create Exclusion Zone"
						closeLabel="Close Create Exclusion Zone"
						onClose={props.onClose}
					/>
					<p>
						Virtual Playbacks {props.playbackNumbers.join(", ")} will be
						mutually exclusive. Creating the zone does not operate any playback.
					</p>
					<FormLayout labelPlacement="side">
						<TextField
							label="Zone name"
							autoFocus
							maxLength={80}
							value={draftName}
							onChange={(event) => {
								setDraftName(event.target.value);
								props.onNameChange(event.target.value);
							}}
						/>
					</FormLayout>
					<footer>
						<Button onClick={props.onClose}>Cancel</Button>
						<Button
							className="primary"
							disabled={
								props.saving ||
								!draftName.trim() ||
								props.playbackNumbers.length < 2
							}
							onClick={() => props.onCreate(draftName)}
						>
							{props.saving ? "Creating…" : "Create zone"}
						</Button>
					</footer>
					{props.error && <p className="modal-error">{props.error}</p>}
				</section>
			</div>
		</ModalRegistration>
	);
}
