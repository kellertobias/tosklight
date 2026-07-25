import { type ReactNode, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ModalTitleBar, Select, TextInput } from "./";
import { Button } from "./controls";

export interface SearchFilter {
	id: string;
	label: string;
	options: string[];
}

export function SearchBar({
	value,
	onChange,
	options,
	filters = [],
	values = {},
	onFilterChange,
	placeholder = "Search",
	ariaLabel = "Search",
	optionsTitle = "Search options",
}: {
	value: string;
	onChange: (value: string) => void;
	/** Custom controls rendered in a stacked options dialog. */
	options?: ReactNode;
	/** Convenience fields for searches whose options are simple selects. */
	filters?: SearchFilter[];
	values?: Record<string, string>;
	onFilterChange?: (id: string, value: string) => void;
	placeholder?: string;
	ariaLabel?: string;
	optionsTitle?: string;
}) {
	const [open, setOpen] = useState(false);
	const optionsButton = useRef<HTMLButtonElement>(null);
	const hasOptions = options != null || filters.length > 0;
	const closeOptions = () => {
		setOpen(false);
		requestAnimationFrame(() => optionsButton.current?.focus());
	};
	useEffect(() => {
		if (!open) return;
		const closeOnEscape = (event: KeyboardEvent) => {
			if (event.key !== "Escape") return;
			event.preventDefault();
			event.stopPropagation();
			closeOptions();
		};
		document.addEventListener("keydown", closeOnEscape, true);
		return () => document.removeEventListener("keydown", closeOnEscape, true);
	}, [open]);
	const filterDialog = (
		<div
			className="stacked-modal-layer search-options-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && closeOptions()
			}
		>
			<section
				className="nested-modal search-filter-modal"
				role="dialog"
				aria-modal="true"
				aria-label={optionsTitle}
			>
				<ModalTitleBar
					title={optionsTitle}
					closeLabel="Close search options"
					onClose={closeOptions}
				/>
				{options ?? filters.map((filter) => (
					<label key={filter.id} htmlFor={`search-filter-${filter.id}`}>
						{filter.label}
						<Select
							id={`search-filter-${filter.id}`}
							value={values[filter.id] ?? ""}
							onChange={(event) =>
								onFilterChange?.(filter.id, event.target.value)
							}
						>
							<option value="">All</option>
							{filter.options.map((option) => (
								<option key={option}>{option}</option>
							))}
						</Select>
					</label>
				))}
				{options == null && <footer>
					<Button
						onClick={() => {
							for (const filter of filters)
								onFilterChange?.(filter.id, "");
						}}
					>
						Clear options
					</Button>
					<Button onClick={closeOptions}>Apply</Button>
				</footer>}
			</section>
		</div>
	);
	return (
		<div className={`console-search ${hasOptions ? "has-options" : ""}`.trim()}>
			<div className="console-search-input">
				{hasOptions ? (
					<Button
						ref={optionsButton}
						iconOnly
						active={open}
						className="console-search-icon console-search-options"
						aria-label="Search options"
						aria-expanded={open}
						onClick={() => setOpen(true)}
					>
						<SearchIcon />
						<span className="console-search-chevron" aria-hidden="true">
							⌄
						</span>
					</Button>
				) : (
					<span className="console-search-icon" aria-hidden="true">
						<SearchIcon />
					</span>
				)}
				<TextInput
					clearable
					clearLabel="Clear search"
					liveKeyboard
					keyboardLabel={ariaLabel}
					aria-label={ariaLabel}
					value={value}
					placeholder={placeholder}
					onChange={(event) => onChange(event.target.value)}
				/>
			</div>
			{open && createPortal(filterDialog, document.body)}
		</div>
	);
}

function SearchIcon() {
	return (
		<svg viewBox="0 0 24 24" aria-hidden="true">
			<circle cx="10.5" cy="10.5" r="6.5" />
			<path d="m15.5 15.5 5 5" />
		</svg>
	);
}
