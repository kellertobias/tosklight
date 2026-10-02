import { ModalPortal, ModalTitleBar } from "@tosklight/ui";
import type { LegacySpecialDialogEntry } from "./specialDialogRegistry";
import type { LegacySpecialDialogHost } from "./useLegacySpecialDialogHost";

/** The legacy Special Dialog card chrome, unchanged from `SpecialDialogsModal`. */
export function LegacySpecialDialogCard({
	family,
	entry,
	host,
	close,
}: {
	family: string;
	entry: LegacySpecialDialogEntry | null;
	host: LegacySpecialDialogHost;
	close(): void;
}) {
	return (
		<ModalPortal onClose={close}>
			<div
				className="modal-backdrop"
				onPointerDown={(event) => {
					if (event.target === event.currentTarget) close();
				}}
			>
				<section
					className={`modal-card special-dialog-card ${entry?.cardClassName ?? ""}`}
				>
					<ModalTitleBar title={`${family} · Special Dialog`} onClose={close} />
					<p>{host.selectedFixtureIds.length} fixtures selected</p>
					{!host.valueWrites.canWrite && (
						<p className="modal-status">Programmer values loading…</p>
					)}
					<div className="special-dialog-content">{entry?.render(host)}</div>
				</section>
			</div>
		</ModalPortal>
	);
}
