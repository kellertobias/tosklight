/**
 * Deleting what the drawing has selected, from the Info panel.
 *
 * The trash button asks first. Shift-clicking it deletes one selected element at once; several are
 * always confirmed, because a stray click on a marquee selection could otherwise empty a rig.
 * Deleting removes the fixture from the show, with every multi-patch copy it has.
 */
import { Button, ModalFrame } from "@tosklight/ui";
import { type MouseEvent, useState } from "react";
import { TauriPatchTransport } from "../document/transport";
import type { CadEntity } from "./types";
import "./cadDeleteSelection.css";

const transport = new TauriPatchTransport();

/** How many names a confirmation lists before it only counts the rest. */
const LISTED_NAMES = 12;

export interface SelectedElement {
	id: string;
	name: string;
	displayId: string;
	/** Placements in the plan: 1, or more for a multi-patched fixture. */
	placements: number;
	/** The profile it was patched from, or the kind of Venue object. */
	model: string;
	/** Its DMX address as the drawing shows it. */
	patch: string;
	/** A lamp, as opposed to a Venue object. */
	isFixture: boolean;
}

/** One entry per selected fixture, in selection order, named as the drawing names it. */
export function selectedElements(
	entities: readonly CadEntity[],
	selectedIds: readonly string[],
): SelectedElement[] {
	return selectedIds.flatMap((id) => {
		const placements = entities.filter((entity) => entity.logicalFixtureId === id);
		const fixture = placements.find((entity) => entity.id === id) ?? placements[0];
		if (!fixture) return [];
		return [
			{
				id,
				name: fixture.name,
				displayId: fixture.fixtureDisplayId,
				placements: new Set(placements.map((entity) => entity.id)).size,
				model: fixture.fixtureProfile ?? fixture.scenery?.kind ?? "",
				patch: fixture.dmxAddress,
				isFixture: fixture.kind !== "venue",
			},
		];
	});
}

export function DeleteSelectionButton({
	count,
	onPress,
}: {
	count: number;
	onPress(event: MouseEvent<HTMLButtonElement>): void;
}) {
	const label = count > 1 ? `Delete ${count} selected elements` : "Delete selected element";
	return (
		<Button
			className="cad-sidebar-delete"
			aria-label={label}
			title={count > 1 ? label : `${label} (Shift-click deletes without asking)`}
			disabled={count === 0}
			onClick={onPress}
		>
			<svg className="cad-sidebar-add-icon" viewBox="0 0 16 16" aria-hidden="true">
				<path d="M2.5 4h11M6 4V2.5h4V4M4 4l.7 9.5h6.6L12 4M6.8 6.5v4.5M9.2 6.5v4.5" />
			</svg>
		</Button>
	);
}

function DeleteConfirm({
	elements,
	deleting,
	onCancel,
	onConfirm,
}: {
	elements: readonly SelectedElement[];
	deleting: boolean;
	onCancel(): void;
	onConfirm(): void;
}) {
	const [first] = elements;
	const many = elements.length > 1;
	const question = many ? `Delete ${elements.length} elements?` : `Delete ${first.name}?`;
	const copies = elements.reduce((sum, element) => sum + element.placements - 1, 0);
	return (
		<ModalFrame
			role="dialog"
			ariaLabel={question}
			dialogClassName="cad-delete-confirm"
			title={question}
			closeLabel="Close without deleting"
			onClose={onCancel}
		>
			<div className="cad-delete-confirm-body">
				{many ? (
					<ul>
						{elements.slice(0, LISTED_NAMES).map((element) => (
							<li key={element.id}>
								<b>{element.displayId}</b> {element.name}
							</li>
						))}
						{elements.length > LISTED_NAMES ? (
							<li>and {elements.length - LISTED_NAMES} more</li>
						) : null}
					</ul>
				) : (
					<p>
						<b>{first.displayId}</b> {first.name} is removed from the show.
					</p>
				)}
				{copies ? (
					<p>
						{copies} multi-patch {copies === 1 ? "copy is" : "copies are"} removed with{" "}
						{many ? "them" : "it"}.
					</p>
				) : null}
				<p>Undo brings {many ? "them" : "it"} back.</p>
				<footer>
					<Button onClick={onCancel}>Cancel</Button>
					<Button className="danger" autoFocus disabled={deleting} onClick={onConfirm}>
						{many ? `Delete all ${elements.length}` : "Delete"}
					</Button>
				</footer>
			</div>
		</ModalFrame>
	);
}

/**
 * The delete flow for one selection: `request` asks (or, with Shift on a single element, deletes at
 * once), and `dialog` is the confirmation to render while it is asking.
 */
export function useDeleteSelection({
	elements,
	onDeleted,
	onError,
}: {
	elements: readonly SelectedElement[];
	onDeleted(): void;
	onError(reason: unknown): void;
}) {
	const [confirming, setConfirming] = useState<readonly SelectedElement[] | null>(null);
	const [deleting, setDeleting] = useState(false);

	async function remove(targets: readonly SelectedElement[]) {
		if (!targets.length) return;
		setDeleting(true);
		try {
			await transport.patchFixtures("", 0, {
				requestId: crypto.randomUUID(),
				fixtures: [],
				removeFixtureIds: targets.map((element) => element.id),
			});
			setConfirming(null);
			onDeleted();
		} catch (reason) {
			onError(reason);
		} finally {
			setDeleting(false);
		}
	}

	function request(event?: { shiftKey?: boolean }) {
		if (!elements.length) return;
		if (event?.shiftKey && elements.length === 1) void remove(elements);
		else setConfirming(elements);
	}

	const dialog = confirming ? (
		<DeleteConfirm
			elements={confirming}
			deleting={deleting}
			onCancel={() => setConfirming(null)}
			onConfirm={() => void remove(confirming)}
		/>
	) : null;
	return { request, dialog };
}

