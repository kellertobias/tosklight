/// <reference types="vite/client" />

import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { useState } from "react";
import type { FixtureBodyModel } from "../wire";

/**
 * The generic body a fixture is drawn as, picked by what it looks like.
 *
 * A name such as "Profile moving light, small" does not tell an operator which of two similar
 * bodies they want, so every choice carries a picture of the shipped model.
 *
 * The pictures are the renders the Model Catalogue appendix already ships, named like the models
 * themselves. They are globbed rather than listed so a body without a render shows a placeholder
 * instead of a broken image. The `-no-clamp` variants are the same body without its hanging clamp
 * and are not what the Stage draws.
 */
const renderUrls = import.meta.glob<string>(
	[
		"../../../../docs/help/assets/models/lamps/*.png",
		"../../../../docs/help/assets/models/av/*.png",
		"!../../../../docs/help/assets/models/**/*-no-clamp.png",
	],
	{ eager: true, query: "?url", import: "default" },
);

const bodyPictures = new Map<string, string>();
for (const [path, url] of Object.entries(renderUrls)) {
	const name = /\/([^/]+)\.png$/u.exec(path)?.[1];
	if (name) bodyPictures.set(name, url);
}

const GUESS_LABEL = "Guess from the fixture type";

function BodyPicture({ id }: { id: string | null }) {
	const url = id ? bodyPictures.get(id) : undefined;
	return (
		<span className="fixture-body-picture" aria-hidden="true">
			{url ? <img src={url} alt="" draggable={false} /> : <span>?</span>}
		</span>
	);
}

export function BodyPickerField({
	value,
	bodyCatalogue,
	onChange,
}: {
	value: string | null;
	bodyCatalogue: FixtureBodyModel[];
	onChange: (value: string | null) => void;
}) {
	const [open, setOpen] = useState(false);
	const chosen = bodyCatalogue.find((body) => body.id === value);
	// A show written by a newer build may name a body this one does not ship; say so, keep it.
	const label = !value
		? GUESS_LABEL
		: chosen
			? `${chosen.group} · ${chosen.label}`
			: `${value} (not shipped with this build)`;
	return (
		<div className="fixture-body-field">
			<span className="fixture-body-field-label">Generic body</span>
			<Button
				className="fixture-body-trigger"
				aria-label={`Generic body: ${label}`}
				aria-haspopup="dialog"
				disabled={bodyCatalogue.length === 0}
				onClick={() => setOpen(true)}
			>
				<BodyPicture id={chosen?.id ?? null} />
				<span>{label}</span>
			</Button>
			{open && (
				<BodyPickerModal
					value={value}
					bodyCatalogue={bodyCatalogue}
					onSelect={(next) => {
						onChange(next);
						setOpen(false);
					}}
					onClose={() => setOpen(false)}
				/>
			)}
		</div>
	);
}

function BodyPickerModal({
	value,
	bodyCatalogue,
	onSelect,
	onClose,
}: {
	value: string | null;
	bodyCatalogue: FixtureBodyModel[];
	onSelect: (value: string | null) => void;
	onClose: () => void;
}) {
	const [query, setQuery] = useState("");
	const needle = query.trim().toLocaleLowerCase();
	const groups: { group: string; bodies: FixtureBodyModel[] }[] = [];
	for (const body of bodyCatalogue) {
		if (
			needle &&
			!`${body.group} ${body.label}`.toLocaleLowerCase().includes(needle)
		)
			continue;
		const last = groups.at(-1);
		if (last && last.group === body.group) last.bodies.push(body);
		else groups.push({ group: body.group, bodies: [body] });
	}
	return (
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer fixture-body-picker-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="nested-modal fixture-body-picker"
					role="dialog"
					aria-modal="true"
					aria-label="Choose generic body"
				>
					<ModalTitleBar
						title="Generic body"
						search={{
							value: query,
							onSearch: setQuery,
							ariaLabel: "Search bodies",
							placeholder: "Search bodies",
						}}
						closeLabel="Close body picker"
						onClose={onClose}
					/>
					<div className="fixture-body-picker-results">
						{!needle && (
							<div role="listbox" aria-label="Fallback">
								<BodyOption
									id={null}
									label={GUESS_LABEL}
									selected={!value}
									onSelect={onSelect}
								/>
							</div>
						)}
						{groups.map(({ group, bodies }) => (
							<section key={group}>
								<h4>{group}</h4>
								<div role="listbox" aria-label={group}>
									{bodies.map((body) => (
										<BodyOption
											key={body.id}
											id={body.id}
											label={body.label}
											selected={body.id === value}
											onSelect={onSelect}
										/>
									))}
								</div>
							</section>
						))}
						{needle && !groups.length && (
							<p role="status">No generic body matches this search.</p>
						)}
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}

function BodyOption({
	id,
	label,
	selected,
	onSelect,
}: {
	id: string | null;
	label: string;
	selected: boolean;
	onSelect: (value: string | null) => void;
}) {
	return (
		<Button
			role="option"
			aria-selected={selected}
			className={`fixture-body-option ${selected ? "is-active" : ""}`.trim()}
			onClick={() => onSelect(id)}
		>
			<BodyPicture id={id} />
			<span>{label}</span>
		</Button>
	);
}
