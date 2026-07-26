import type { ReactNode } from "react";
import {
	SearchBar,
	type SearchFeatureProps,
} from "./SearchBar";

export interface ModalTitleTab {
	id: string;
	label: ReactNode;
	disabled?: boolean;
}

export function TitleBarSearchDivider() {
	return <span className="ui-titlebar-search-divider" aria-hidden="true" />;
}

export type ModalTitleBarProps = {
	title: ReactNode;
	details?: ReactNode;
	tabs?: ModalTitleTab[];
	activeTab?: string;
	onTabChange?: (id: string) => void;
	actions?: ReactNode;
	onClose?: () => void;
	closeDisabled?: boolean;
	closeLabel?: string;
	className?: string;
} & SearchFeatureProps;

export function ModalTitleBar({
	title,
	details,
	tabs,
	activeTab,
	onTabChange,
	actions,
	onClose,
	closeDisabled = false,
	closeLabel = "Close modal",
	className = "",
	onSearch,
	search,
}: ModalTitleBarProps) {
	const hasTabs = Boolean(tabs?.length);
	return (
		<header className={`ui-modal-titlebar ${className}`.trim()}>
			<h2 className="ui-modal-title-heading">{title}</h2>
			{details && <div className="ui-modal-title-details">{details}</div>}
			<span className="ui-modal-title-spacer" />
			{hasTabs && (
				<div className="ui-modal-title-tabs" role="tablist">
					{tabs?.map((tab) => (
						<button
							type="button"
							key={tab.id}
							role="tab"
							aria-selected={tab.id === activeTab}
							className={`ui-button ui-secondary ui-default ${tab.id === activeTab ? "active" : ""}`.trim()}
							disabled={tab.disabled}
							onClick={() => onTabChange?.(tab.id)}
						>
							{tab.label}
						</button>
					))}
				</div>
			)}
			{hasTabs && onSearch && search && <TitleBarSearchDivider />}
			{onSearch && search && (
				<div className="ui-modal-title-search">
					<SearchBar
						{...search}
						ariaLabel={search.ariaLabel ?? titleSearchLabel(title)}
						settingsTitle={
							search.settingsTitle ?? titleSettingsLabel(title)
						}
						onChange={onSearch}
					/>
				</div>
			)}
			{onSearch && search && (actions || onClose) && <TitleBarSearchDivider />}
			{actions && <div className="ui-modal-title-actions">{actions}</div>}
			{onClose && (
				<button
					type="button"
					className="ui-button ui-secondary ui-default ui-modal-title-close"
					aria-label={closeLabel}
					title={closeLabel}
					disabled={closeDisabled}
					onClick={onClose}
				>
					×
				</button>
			)}
		</header>
	);
}

function titleSearchLabel(title: ReactNode) {
	return typeof title === "string" ? `Search ${title}` : "Search";
}

function titleSettingsLabel(title: ReactNode) {
	return typeof title === "string" ? `${title} search settings` : "Search settings";
}
