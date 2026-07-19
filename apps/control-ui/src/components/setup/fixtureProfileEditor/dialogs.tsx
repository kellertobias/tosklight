import { Button, ModalTitleBar, SearchBar } from "../../common";

export function ManufacturerLookup({
	manufacturers,
	query,
	onQuery,
	onSelect,
	onClose,
}: {
	manufacturers: string[];
	query: string;
	onQuery: (value: string) => void;
	onSelect: (value: string) => void;
	onClose: () => void;
}) {
	const unique = new Map<string, string>();
	for (const manufacturer of manufacturers)
		if (!unique.has(manufacturer.toLocaleLowerCase()))
			unique.set(manufacturer.toLocaleLowerCase(), manufacturer);
	const matches = [...unique.values()]
		.filter((value) =>
			value.toLocaleLowerCase().includes(query.toLocaleLowerCase()),
		)
		.sort((left, right) => left.localeCompare(right));
	return (
		<div
			className="stacked-modal-layer manufacturer-lookup-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onClose()
			}
		>
			<section
				className="nested-modal manufacturer-lookup"
				role="dialog"
				aria-modal="true"
				aria-label="Manufacturer lookup"
			>
				<ModalTitleBar
					title="Manufacturer lookup"
					search={
						<SearchBar
							value={query}
							onChange={onQuery}
							ariaLabel="Search manufacturers"
							placeholder="Search manufacturers"
						/>
					}
					closeLabel="Close manufacturer lookup"
					onClose={onClose}
				/>
				<div
					className="manufacturer-results"
					role="listbox"
					aria-label="Manufacturers"
				>
					{matches.map((manufacturer) => (
						<Button
							role="option"
							key={manufacturer}
							onClick={() => onSelect(manufacturer)}
						>
							{manufacturer}
						</Button>
					))}
					{!matches.length && (
						<p>
							No manufacturer matches this search. Close the lookup to keep
							typing a new manufacturer.
						</p>
					)}
				</div>
			</section>
		</div>
	);
}

export function ConfirmDialog({
	title,
	description,
	primary,
	secondary,
	danger = false,
	onPrimary,
	onSecondary,
}: {
	title: string;
	description: string;
	primary: string;
	secondary: string;
	danger?: boolean;
	onPrimary: () => void;
	onSecondary: () => void;
}) {
	return (
		<div className="stacked-modal-layer fixture-confirm-layer">
			<section
				className="nested-modal fixture-confirm-dialog"
				role="alertdialog"
				aria-modal="true"
				aria-label={title}
			>
				<ModalTitleBar title={title} />
				<p>{description}</p>
				<div className="modal-actions">
					<Button autoFocus onClick={onSecondary}>
						{secondary}
					</Button>
					<Button variant={danger ? "danger" : "primary"} onClick={onPrimary}>
						{primary}
					</Button>
				</div>
			</section>
		</div>
	);
}
