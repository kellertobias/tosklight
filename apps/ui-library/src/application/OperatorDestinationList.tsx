import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Button } from "../controls";

export interface OperatorDestination {
	id: string;
	label: string;
	icon: ReactNode;
	disabled?: boolean;
	buttonProps?: Omit<
		ButtonHTMLAttributes<HTMLButtonElement>,
		"children" | "disabled" | "onClick"
	>;
}

/**
 * The destination list used by the Light Desk dock. Products supply their own
 * destinations, while the control geometry and active-state semantics stay shared.
 */
export function OperatorDestinationList({
	ariaLabel,
	entries,
	activeId,
	onSelect,
	className = "",
}: {
	ariaLabel: string;
	entries: readonly OperatorDestination[];
	activeId?: string;
	onSelect: (id: string) => void;
	className?: string;
}) {
	return (
		<nav
			className={`dock-list ui-operator-destination-list ${className}`.trim()}
			aria-label={ariaLabel}
		>
			{entries.map((entry) => (
				<Button
					{...entry.buttonProps}
					key={entry.id}
					className={`dock-entry ui-operator-destination ${entry.id === activeId ? "active" : ""}`}
					aria-label={entry.label}
					aria-current={entry.id === activeId ? "page" : undefined}
					active={entry.id === activeId}
					disabled={entry.disabled}
					contentAlign="center"
					onClick={() => onSelect(entry.id)}
				>
					<span className="dock-entry-icon" aria-hidden="true">
						{entry.icon}
					</span>
					<span className="dock-entry-label">{entry.label}</span>
				</Button>
			))}
		</nav>
	);
}
