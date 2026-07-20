import { Button, FormLayout, TextField } from "../components/common";
import { VirtualPlaybackConfigurationModal } from "../components/control/VirtualPlaybackConfigurationModal";
import { VirtualPlaybackGrid } from "../components/control/virtualPlayback/VirtualPlaybackGrid";
import { useVirtualPlaybackController } from "../components/control/virtualPlayback/useVirtualPlaybackController";
import type { WindowProps } from "./windowTypes";

export function VirtualPlaybacksWindow({ paneId, active = true }: WindowProps) {
	const controller = useVirtualPlaybackController(paneId, active);
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
			<VirtualPlaybackToolbar
				pageNumber={controller.pageNumber}
				rows={controller.rows}
				columns={controller.columns}
				zonesReady={controller.zones.ready}
				zoneError={controller.zones.error}
				actionError={controller.topologyActionError}
				zoneCount={controller.zones.zones.length}
				selectedSlots={controller.selectedSlots}
				onSetSource={() => {
					controller.dispatch({ type: "SET_CUELIST_SET_TARGET", value: null });
					controller.dispatch({ type: "SET_CUELIST_SET_ARMED", value: true });
				}}
				onAddTarget={() =>
					controller.dispatch({ type: "SET_CUELIST_SET_ARMED", value: true })
				}
				onCreateZone={(name) => {
					controller.setZoneName(name);
					controller.setCreatingZone(true);
				}}
				onCancelZone={() => {
					controller.setSelectedSlots([]);
					controller.dispatch({ type: "SET_SHIFT_ARMED", value: false });
				}}
			/>
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
				assignmentPending={controller.assignmentPending}
				assignmentTarget={controller.state.cueListSetTarget}
				updateArmed={controller.state.updateArmed}
				shiftArmed={controller.state.shiftArmed}
				onConfigure={controller.openConfiguration}
				onAssign={(slot) => void controller.assignSource(slot)}
				onToggleZone={controller.toggleZoneSlot}
			/>
			{controller.configuration && (
				<VirtualPlaybackConfigurationModal
					playback={controller.configuration.playback}
					page={controller.pageNumber}
					slot={controller.configuration.slot}
					empty={controller.configuration.empty}
					expectedPageRevision={
						controller.configuration.expectedPageRevision
					}
					expectedPageObjectId={
						controller.configuration.expectedPageObjectId
					}
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
					onNameChange={controller.setZoneName}
					onClose={() => controller.setCreatingZone(false)}
					onCreate={() => void controller.createZone()}
				/>
			)}
		</section>
	);
}

function VirtualPlaybackToolbar(props: {
	pageNumber: number;
	rows: number;
	columns: number;
	zonesReady: boolean;
	zoneError: string | null;
	actionError: string | null;
	zoneCount: number;
	selectedSlots: readonly number[];
	onSetSource(): void;
	onAddTarget(): void;
	onCreateZone(name: string): void;
	onCancelZone(): void;
}) {
	return (
		<header className="virtual-playback-toolbar">
			<Button onClick={props.onSetSource}>Set Source</Button>
			<Button onClick={props.onAddTarget}>Add Target</Button>
			{props.zonesReady && props.selectedSlots.length >= 2 && (
				<Button
					className="primary"
					onClick={() =>
						props.onCreateZone(`Exclusion Zone ${props.zoneCount + 1}`)
					}
				>
					Create Exclusion Zone
				</Button>
			)}
			{props.selectedSlots.length > 0 && (
				<Button onClick={props.onCancelZone}>Cancel zone selection</Button>
			)}
			<span>
				{props.selectedSlots.length > 0
					? `${props.selectedSlots.length} cells selected · `
					: ""}
				Page {props.pageNumber} · {props.rows}×{props.columns}
				{!props.zonesReady && !props.zoneError ? " · Loading zones…" : ""}
			</span>
			{props.zoneError && <span role="alert">{props.zoneError}</span>}
			{props.actionError && <span role="alert">{props.actionError}</span>}
		</header>
	);
}

function CreateZoneModal(props: {
	selectedSlots: readonly number[];
	name: string;
	error: string | null;
	saving: boolean;
	onNameChange(name: string): void;
	onClose(): void;
	onCreate(): void;
}) {
	return (
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
				<Button className="modal-close" onClick={props.onClose}>×</Button>
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
						value={props.name}
						onChange={(event) => props.onNameChange(event.target.value)}
					/>
				</FormLayout>
				<footer>
					<Button onClick={props.onClose}>Cancel</Button>
					<Button
						className="primary"
						disabled={
							props.saving ||
							!props.name.trim() ||
							props.selectedSlots.length < 2
						}
						onClick={props.onCreate}
					>
						{props.saving ? "Creating…" : "Create zone"}
					</Button>
				</footer>
				{props.error && <p className="modal-error">{props.error}</p>}
			</section>
		</div>
	);
}
