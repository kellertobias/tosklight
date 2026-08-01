import { ModalPortal, ModalTitleBar } from "@tosklight/ui";
import type { ProgrammerLifecycleRow } from "../../../features/programmerLifecycle/contracts";
import { ProgrammerList } from "./ProgrammerList";

interface ActiveProgrammersModalProps {
	open: boolean;
	programmers: readonly ProgrammerLifecycleRow[];
	loading: boolean;
	currentUserId: string | null;
	currentUserName: string | null;
	onClear(sessionId: string): void;
	onClose(): void;
}

export function ActiveProgrammersModal(props: ActiveProgrammersModalProps) {
	if (!props.open) return null;
	return (
		<ModalPortal onClose={props.onClose}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) => {
					if (event.target === event.currentTarget) props.onClose();
				}}
			>
				<section
					className="nested-modal active-programmers-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Active Programmers"
				>
					<ModalTitleBar
						className="active-programmers-titlebar"
						title="Active Programmers"
						details={`${props.programmers.length} active`}
						closeLabel="Close Active Programmers"
						onClose={props.onClose}
					/>
					<ProgrammerList
						programmers={props.programmers}
						loading={props.loading}
						currentUserId={props.currentUserId}
						currentUserName={props.currentUserName}
						onClear={props.onClear}
					/>
				</section>
			</div>
		</ModalPortal>
	);
}
