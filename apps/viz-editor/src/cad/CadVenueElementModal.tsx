/**
 * The dialog behind **Add venue element**: the Venue profiles no add button of its own offers, each
 * shown by its picture on a dark ground, in a scrolling grid the title's search narrows.
 *
 * A Venue profile is one from the Venue manufacturer or any profile placed without DMX, so imported
 * venue models are listed beside the shipped railings, crowds, mirror balls, PA and figures. The
 * trusses, decks, curtains and primitives are left out: they are placed from their own buttons and
 * part menus. Choosing one selects it and **Add** (or a double-click) places one; the dialog stays
 * open with the show's reason when the show refuses it. Each element's **Add Several** button closes
 * the dialog and holds that element, so every press on a viewport places another copy.
 */
import { ModalFrame } from "@tosklight/ui";
import { useState } from "react";
import { type FixtureLibrary, useFixtureLibrary } from "./cadPlacement";
import { matchesVenueQuery, previewOf, venueProfiles } from "./venueParts";
import "./cadAddParts.css";

const TITLE = "Add venue element";

export function CadVenueElementModal({
	placing,
	onChoose,
	onAddSeveral,
	onClose,
}: {
	placing: boolean;
	/** Places one of the element. */
	onChoose(profileId: string, name: string, library: FixtureLibrary): void;
	/** Holds the element for repeated placement on the viewports. */
	onAddSeveral(profileId: string, name: string): void;
	onClose(): void;
}) {
	const library = useFixtureLibrary();
	const [query, setQuery] = useState("");
	const [selected, setSelected] = useState<string | null>(null);
	const all = library.state === "ready" ? venueProfiles(library.definitions) : [];
	const shown = all.filter(({ definition }) => matchesVenueQuery(definition, query));
	const chosen = shown.find(({ profileId }) => profileId === selected);
	const add = () => {
		if (chosen) onChoose(chosen.profileId, chosen.definition.name, library);
	};
	return (
		<ModalFrame
			ariaLabel={TITLE}
			dialogClassName="cad-add-part-modal"
			title={TITLE}
			closeLabel={`Close ${TITLE}`}
			search={{
				value: query,
				onSearch: setQuery,
				ariaLabel: "Search venue elements",
				placeholder: "Search venue elements",
			}}
			accept={{
				id: "add-venue-element",
				label: "Add",
				ariaLabel: chosen ? `Add ${chosen.definition.name}` : "Add the selected venue element",
				disabled: !chosen || placing,
				onPress: add,
			}}
			onClose={onClose}
		>
			<div className="cad-add-part-body" aria-busy={placing || library.state === "loading" || undefined}>
				{library.state === "loading" ? (
					<p className="cad-add-part-note">Loading the fixture library…</p>
				) : null}
				{library.state === "failed" ? (
					<p className="cad-add-part-note" role="alert">
						The fixture library could not be read: {library.reason}
					</p>
				) : null}
				{library.state === "ready" && shown.length === 0 ? (
					<p className="cad-add-part-note">
						{all.length
							? `No venue element matches “${query.trim()}”.`
							: "This computer's fixture library holds no venue elements."}
					</p>
				) : null}
				<div className="cad-part-grid" role="list" aria-label="Venue elements">
					{shown.map(({ profileId, definition }) => {
						const preview = previewOf(definition);
						return (
							<div role="listitem" key={profileId} className="cad-part-item">
								<button
									type="button"
									className="cad-part-tile"
									aria-pressed={profileId === selected}
									disabled={placing}
									onClick={() => setSelected(profileId)}
									onDoubleClick={() => onChoose(profileId, definition.name, library)}
								>
									<span className="cad-part-preview">
										{preview ? (
											<img src={preview} alt="" />
										) : (
											<span aria-hidden="true">No picture</span>
										)}
									</span>
									<strong>{definition.name}</strong>
									<small>
										{definition.profile_snapshot?.fixture_type === "rigging" ? "Rigging" : "Venue"}
									</small>
								</button>
								<button
									type="button"
									className="cad-part-add-several"
									title="Add Several"
									aria-label={`Add Several ${definition.name}`}
									disabled={placing}
									onClick={() => onAddSeveral(profileId, definition.name)}
								>
									<span aria-hidden="true">++</span>
								</button>
							</div>
						);
					})}
				</div>
			</div>
		</ModalFrame>
	);
}
