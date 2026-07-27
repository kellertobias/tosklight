import type { ReactNode } from "react";
import { Button } from "../common/controls/foundation";

export interface EncoderGroupTab<T extends string = string> {
	id: T;
	label: string;
	compactLabel?: string;
	pageCount?: number;
}

export interface EncoderGroupTabsProps<T extends string = string> {
	groups: readonly EncoderGroupTab<T>[];
	activeGroup: T;
	page: number;
	onChange(group: T, page: number): void;
	trailing?: ReactNode;
	className?: string;
}

/**
 * Shared encoder-group navigation. Selecting a different group opens its first
 * page; selecting the active group advances through its pages and wraps.
 */
export function EncoderGroupTabs<T extends string>({
	groups,
	activeGroup,
	page,
	onChange,
	trailing,
	className = "",
}: EncoderGroupTabsProps<T>) {
	return (
		<div
			className={`encoder-group-tabs ${className}`.trim()}
			data-encoder-group={activeGroup}
			data-encoder-page={page}
		>
			{groups.map((group) => {
				const pageCount = Math.max(1, group.pageCount ?? 1);
				const active = group.id === activeGroup;
				const visiblePage = active ? Math.min(Math.max(page, 1), pageCount) : 1;
				const fullLabel =
					active && pageCount > 1
						? `${group.label} (${visiblePage}/${pageCount})`
						: group.label;
				const compactBase = group.compactLabel ?? group.label;
				const compactLabel =
					active && pageCount > 1
						? `${compactBase} (${visiblePage}/${pageCount})`
						: compactBase;
				return (
					<Button
						key={group.id}
						aria-label={fullLabel}
						active={active}
						onClick={() =>
							onChange(
								group.id,
								active && pageCount > 1 ? (visiblePage % pageCount) + 1 : 1,
							)
						}
					>
						<span className="family-label-full" aria-hidden="true">
							{fullLabel}
						</span>
						<span className="family-label-compact" aria-hidden="true">
							{compactLabel}
						</span>
					</Button>
				);
			})}
			{trailing != null && (
				<>
					<span className="family-spacer" />
					{trailing}
				</>
			)}
		</div>
	);
}
