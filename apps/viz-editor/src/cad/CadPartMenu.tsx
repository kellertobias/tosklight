/**
 * The menu behind the caret of **Add truss**, **Add stage element**, **Add curtain** and
 * **Add primitive**: the button's parts, each with its picture on a dark ground.
 *
 * A group with several parts — a truss section's corner pieces, a stage's platform sizes — is headed
 * by its name; a group with one part is that part. The part the button places now is checked.
 * Choosing a part makes it the button's part and places it. A part this computer's library does not
 * hold is listed but cannot be chosen.
 */
import { chosenPart } from "./cadAddChoice";
import { type FixtureLibrary, useFixtureLibrary } from "./cadPlacement";
import {
	CAD_PART_CATALOGUE,
	type CadPartKind,
	definitionForProfile,
	previewOf,
	type VenuePart,
} from "./venueParts";
import "./cadAddParts.css";

function PartMenuItem({
	label,
	detail,
	part,
	library,
	checked,
	onChoose,
}: {
	label: string;
	detail: string | undefined;
	part: VenuePart;
	library: FixtureLibrary;
	checked: boolean;
	onChoose(profileId: string): void;
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
		<button
			type="button"
			role="menuitemradio"
			aria-checked={checked}
			className="cad-part-menu-item"
			disabled={!definition}
			onClick={() => onChoose(part.profileId)}
		>
			<span className="cad-part-menu-preview">
				{preview ? <img src={preview} alt="" /> : null}
			</span>
			<span className="cad-part-menu-text">
				<strong>{label}</strong>
				{note ? <small>{note}</small> : null}
			</span>
		</button>
	);
}

export function CadPartMenu({
	kind,
	onChoose,
}: {
	kind: CadPartKind;
	onChoose(profileId: string): void;
}) {
	const library = useFixtureLibrary();
	const current = chosenPart(kind).part.profileId;
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
						detail={detail}
						part={part}
						library={library}
						checked={part.profileId === current}
						onChoose={onChoose}
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
		</div>
	);
}
