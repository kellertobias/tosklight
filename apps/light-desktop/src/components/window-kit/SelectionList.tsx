import type { ReactNode } from "react";
import { Button } from "../common";
import { WindowScrollArea } from "./WindowKit";

export interface SelectionListOption {
	value: string;
	label: ReactNode;
	description?: ReactNode;
	disabled?: boolean;
	tone?: "default" | "danger";
}

export function SelectionList({
	ariaLabel,
	value,
	options,
	onChange,
	emptyLabel = "No options are available",
	className = "",
}: {
	ariaLabel: string;
	value?: string;
	options: SelectionListOption[];
	onChange: (value: string) => void;
	emptyLabel?: ReactNode;
	className?: string;
}) {
	return (
		<WindowScrollArea className={`ui-selection-list-scroll ${className}`.trim()}>
			<div className="ui-selection-list" role="radiogroup" aria-label={ariaLabel}>
				{options.length > 0 ? options.map((option) => (
					<Button
						className="ui-selection-list-option"
						variant={option.tone === "danger" ? "danger" : "secondary"}
						role="radio"
						aria-checked={option.value === value}
						active={option.value === value}
						disabled={option.disabled}
						key={option.value}
						onClick={() => onChange(option.value)}
					>
						<span>{option.label}</span>
						{option.description != null && <small>{option.description}</small>}
					</Button>
				)) : <div className="ui-selection-list-option ui-selection-list-empty" role="status">{emptyLabel}</div>}
			</div>
		</WindowScrollArea>
	);
}

export interface SelectionTreeColumn {
	id: string;
	title: ReactNode;
	ariaLabel: string;
	value?: string;
	options: SelectionListOption[];
	onChange: (value: string) => void;
	emptyLabel?: ReactNode;
	footer?: ReactNode;
	className?: string;
}

export function SelectionTree({ columns, className = "" }: { columns: SelectionTreeColumn[]; className?: string }) {
	return <div className={`ui-selection-tree ${className}`.trim()}>{columns.map((column) => <section className={column.className} key={column.id}>
		<h3>{column.title}</h3>
		<SelectionList ariaLabel={column.ariaLabel} value={column.value} options={column.options} onChange={column.onChange} emptyLabel={column.emptyLabel}/>
		{column.footer != null && <div className="ui-selection-tree-footer">{column.footer}</div>}
	</section>)}</div>;
}
