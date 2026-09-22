/**
 * The dialog behind **Add venue element**: the Venue profiles no add button of its own offers, each
 * shown by its picture on a dark ground, in a scrolling grid the title's search narrows.
 *
 * A Venue profile is one from the Venue manufacturer or any profile placed without DMX, so imported
 * venue models are listed beside the shipped railings, crowds, mirror balls, PA and figures. The
 * trusses, decks, curtains and primitives are left out: they are placed from their own buttons and
 * part menus. Choosing one places it; the dialog stays open with the show's reason when the show
 * refuses it.
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
	onClose,
}: {
	placing: boolean;
	onChoose(profileId: string, name: string, library: FixtureLibrary): void;
	onClose(): void;
}) {
	const library = useFixtureLibrary();
	const [query, setQuery] = useState("");
	const all = library.state === "ready" ? venueProfiles(library.definitions) : [];
	const shown = all.filter(({ definition }) => matchesVenueQuery(definition, query));
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
							<div role="listitem" key={profileId}>
								<button
									type="button"
									className="cad-part-tile"
									disabled={placing}
									onClick={() => onChoose(profileId, definition.name, library)}
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
							</div>
						);
					})}
				</div>
			</div>
		</ModalFrame>
	);
}
