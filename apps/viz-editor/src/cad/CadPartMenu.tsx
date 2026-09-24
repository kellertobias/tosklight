/**
 * The menu behind the caret of **Add truss**, **Add stage element**, **Add scenery** and
 * **Add primitive**: the button's parts, each with its picture on a dark ground.
 *
 * A group with several parts — a truss section's corner pieces, a stage's platform sizes — is headed
 * by its name; a group with one part is that part. The part the button places now is checked.
 * Choosing a part makes it the button's part and places it. A part this computer's library does not
 * hold is listed but cannot be chosen.
 *
 * Every part's row carries **Add Several** at its right edge: it holds that part so each press on a
 * viewport places one more copy, without placing one first. A truss and a stage element are rarely
 * placed one at a time, so those two menus also end with **Place several…**, which opens the wizard
 * for the part the button places now.
 */
import { chosenPart } from "./cadAddChoice";
import { type FixtureLibrary, useFixtureLibrary } from "./cadPlacement";
import {
	CAD_PART_CATALOGUE,
	type CadPartKind,
	definitionForProfile,
	partKey,
	partLabel,
	previewOf,
	type VenuePart,
} from "./venueParts";
import "./cadAddParts.css";

function PartMenuItem({
	label,
	name,
	detail,
	part,
	library,
	checked,
	onChoose,
	onAddSeveral,
}: {
	label: string;
	/** The part's full name, its group's and its own, for the Add Several button. */
	name: string;
	detail: string | undefined;
	part: VenuePart;
	library: FixtureLibrary;
	checked: boolean;
	onChoose(profileId: string): void;
	onAddSeveral?(profileId: string): void;
}) {
	const definition =
		library.state === "ready" ? definitionForProfile(library.definitions, part.profileId) : undefined;
	const preview = previewOf(definition);
	const note =
		library.state === "loading"
			? "Loading…"
			: library.state === "failed" || !definition
				? "Not in this library"
				: detail;
	return (
		<div className="cad-part-menu-row">
			<button
				type="button"
				role="menuitemradio"
				aria-checked={checked}
				className="cad-part-menu-item"
				disabled={!definition}
				onClick={() => onChoose(partKey(part))}
			>
				<span className="cad-part-menu-preview">
					{preview ? <img src={preview} alt="" /> : null}
				</span>
				<span className="cad-part-menu-text">
					<strong>{label}</strong>
					{note ? <small>{note}</small> : null}
				</span>
			</button>
			{onAddSeveral ? (
				<button
					type="button"
					role="menuitem"
					className="cad-part-add-several cad-part-menu-add-several"
					title="Add Several"
					aria-label={`Add Several ${name}`}
					disabled={!definition}
					onClick={() => onAddSeveral(partKey(part))}
				>
					<span aria-hidden="true">++</span>
				</button>
			) : null}
		</div>
	);
}

export function CadPartMenu({
	kind,
	onChoose,
	onAddSeveral,
	onSeveral,
	onLoadModel,
}: {
	kind: CadPartKind;
	onChoose(profileId: string): void;
	/** Holds a part for repeated placement: each press on a viewport places one more copy. */
	onAddSeveral?(profileId: string): void;
	/** Opens the bulk wizard for the part the button places now; absent on kinds that have none. */
	onSeveral?(profileId: string): void;
	/** Loads a 3D model file instead of a part from the library; absent on kinds that have none. */
	onLoadModel?(): void;
}) {
	const library = useFixtureLibrary();
	const current = partKey(chosenPart(kind).part);
	return (
		<div className="cad-part-menu" aria-busy={library.state === "loading" || undefined}>
			{library.state === "failed" ? (
				<p className="cad-part-menu-note" role="alert">
					The fixture library could not be read: {library.reason}
				</p>
			) : null}
			{CAD_PART_CATALOGUE[kind].map((group) => {
				const item = (part: VenuePart, label: string, detail: string | undefined) => (
					<PartMenuItem
						key={part.id}
						label={label}
						name={partLabel({ group, part })}
						detail={detail}
						part={part}
						library={library}
						checked={partKey(part) === current}
						onChoose={onChoose}
						onAddSeveral={onAddSeveral}
					/>
				);
				if (group.parts.length === 1) {
					const [part] = group.parts;
					return item(part, group.label, part.detail ?? part.label);
				}
				return (
					<div key={group.id} role="group" aria-label={group.label} className="cad-part-menu-group">
						<span className="cad-part-menu-heading" aria-hidden="true">
							{group.label}
						</span>
						{group.parts.map((part) => item(part, part.label, part.detail))}
					</div>
				);
			})}
			{onLoadModel ? (
				<button
					type="button"
					role="menuitem"
					className="cad-part-menu-item cad-part-menu-several"
					onClick={onLoadModel}
				>
					<span className="cad-part-menu-text">
						<strong>Load model…</strong>
						<small>A glTF, GLB, 3MF or OBJ file</small>
					</span>
				</button>
			) : null}
			{onSeveral ? (
				<button
					type="button"
					role="menuitem"
					className="cad-part-menu-item cad-part-menu-several"
					onClick={() => onSeveral(current)}
				>
					<span className="cad-part-menu-text">
						<strong>Place several…</strong>
						<small>A field of them, laid out at once</small>
					</span>
				</button>
			) : null}
		</div>
	);
}
