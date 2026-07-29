import {
	Button,
	FormLayout,
	ModalRegistration,
	TextField,
} from "@tosklight/ui";
import { useState } from "react";
import { createPortal } from "react-dom";
import { VirtualPlaybackConfigurationModal } from "../components/control/VirtualPlaybackConfigurationModal";
import { useVirtualPlaybackController } from "../components/control/virtualPlayback/useVirtualPlaybackController";
import { VirtualPlaybackGrid } from "../components/control/virtualPlayback/VirtualPlaybackGrid";
import { usePaneChromeTargets } from "../components/shell/PaneChromeContext";
import type { WindowProps } from "./windowTypes";

export function VirtualPlaybacksWindow({ paneId, active = true }: WindowProps) {
	const controller = useVirtualPlaybackController(paneId, active);
	const paneChrome = usePaneChromeTargets();
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
				rows={controller.rows}
				columns={controller.columns}
				playbacks={controller.playbacks}
				cueLists={controller.cueLists}
				runtimes={controller.runtimes}
				runtimeActions={controller.runtimeActions}
				zones={controller.zones.zones}
				selectedSlots={controller.selectedSlots}
				configurationArmed={controller.configurationArmed}
				updateArmed={controller.state.updateArmed}
				shiftArmed={controller.state.shiftArmed}
				onConfigure={controller.openConfiguration}
				onToggleZone={controller.toggleZoneSlot}
				paneId={paneId}
			/>
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
					selectedSlots={controller.selectedSlots}
					name={controller.zoneName}
					error={controller.zones.error}
					saving={controller.zones.saving}
					onClose={() => controller.setCreatingZone(false)}
					onCreate={(name) => void controller.createZone(name)}
				/>
			)}
		</section>
	);
}

export function VirtualPlaybackTitleActions(props: {
	zonesReady: boolean;
	saving: boolean;
	zoneCount: number;
	selectedSlots: readonly number[];
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
							props.selectedSlots.length < 2
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
	selectedSlots: readonly number[];
	name: string;
	error: string | null;
	saving: boolean;
	onClose(): void;
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
					<Button className="modal-close" onClick={props.onClose}>
						×
					</Button>
					<h3>Create Exclusion Zone</h3>
					<p>
						Cells {props.selectedSlots.join(", ")} on the current page will be
						mutually exclusive. Creating the zone does not operate any playback.
					</p>
					<FormLayout labelPlacement="side">
						<TextField
							label="Zone name"
							autoFocus
							maxLength={80}
							value={draftName}
							onChange={(event) => setDraftName(event.target.value)}
						/>
					</FormLayout>
					<footer>
						<Button onClick={props.onClose}>Cancel</Button>
						<Button
							className="primary"
							disabled={
								props.saving ||
								!draftName.trim() ||
								props.selectedSlots.length < 2
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
