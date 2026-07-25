import type { FixtureMode, FixtureProfile } from "../../../api/types";
import { Button, LargeTextField, TextField } from "../../common";

export function ModesTab({
	draft,
	onChange,
	onMove,
	onDelete,
	onEdit,
}: {
	draft: FixtureProfile;
	onChange: (mode: FixtureMode) => void;
	onMove: (sourceId: string, targetId: string) => void;
	onDelete: (id: string) => void;
	onEdit: (mode: FixtureMode) => void;
}) {
	return (
		<div className="fixture-modes-tab">
			<section className="fixture-mode-list">
				{draft.modes.map((mode) => (
					<article key={mode.id} data-mode-reorder-id={mode.id}>
						<span
							className="drag-handle touch-drag-handle"
							aria-hidden="true"
							title="Drag to reorder modes"
							onPointerDown={(event) => {
								event.preventDefault();
								event.currentTarget.setPointerCapture(event.pointerId);
							}}
							onPointerMove={(event) => {
								if (!event.currentTarget.hasPointerCapture(event.pointerId))
									return;
								const target = document
									.elementFromPoint(event.clientX, event.clientY)
									?.closest<HTMLElement>("[data-mode-reorder-id]")
									?.dataset.modeReorderId;
								if (target) onMove(mode.id, target);
							}}
							onPointerUp={(event) =>
								event.currentTarget.hasPointerCapture(event.pointerId) &&
								event.currentTarget.releasePointerCapture(event.pointerId)
							}
							onPointerCancel={(event) =>
								event.currentTarget.hasPointerCapture(event.pointerId) &&
								event.currentTarget.releasePointerCapture(event.pointerId)
							}
						>
							⠿
						</span>
						<TextField
							label="Mode name"
							required
							value={mode.name}
							onChange={(event) =>
								onChange({ ...mode, name: event.target.value })
							}
						/>
						<Button
							className="fixture-mode-edit"
							aria-label={`Edit channels for ${mode.name || "unnamed mode"}`}
							onClick={() => onEdit(mode)}
						>
							Edit
						</Button>
						<Button
							className="fixture-mode-delete"
							variant="danger"
							iconOnly
							aria-label={`Remove ${mode.name}`}
							disabled={draft.modes.length === 1}
							title={
								draft.modes.length === 1
									? "The final mode cannot be removed"
									: "Remove mode"
							}
							onClick={() => onDelete(mode.id)}
						>
							<svg aria-hidden="true" viewBox="0 0 24 24">
								<path d="M4 7h16M9 7V4h6v3m-9 0 1 13h10l1-13M10 11v5m4-5v5" />
							</svg>
						</Button>
						<LargeTextField
							className="fixture-mode-notes"
							label="Mode notes"
							value={mode.notes}
							onChange={(event) =>
								onChange({ ...mode, notes: event.target.value })
							}
						/>
					</article>
				))}
			</section>
		</div>
	);
}
