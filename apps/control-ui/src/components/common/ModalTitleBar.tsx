import type { ReactNode } from "react";

export interface ModalTitleTab {
	id: string;
	label: ReactNode;
	disabled?: boolean;
}

export function ModalTitleBar({
	title,
	details,
	tabs,
	activeTab,
	onTabChange,
	search,
	actions,
	onClose,
	closeLabel = "Close modal",
	className = "",
}: {
	title: ReactNode;
	details?: ReactNode;
	tabs?: ModalTitleTab[];
	activeTab?: string;
	onTabChange?: (id: string) => void;
	search?: ReactNode;
	actions?: ReactNode;
	onClose?: () => void;
	closeLabel?: string;
	className?: string;
}) {
	const hasTabs = Boolean(tabs?.length);
	return (
		<header className={`ui-modal-titlebar ${className}`.trim()}>
			<h2 className="ui-modal-title-heading">{title}</h2>
			{details && <div className="ui-modal-title-details">{details}</div>}
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
			<span className="ui-modal-title-spacer" />
			{search && <div className="ui-modal-title-search">{search}</div>}
			{actions && <div className="ui-modal-title-actions">{actions}</div>}
			{onClose && (
				<button
					type="button"
					className="ui-button ui-secondary ui-default ui-modal-title-close"
					aria-label={closeLabel}
					title={closeLabel}
					onClick={onClose}
				>
					×
				</button>
			)}
		</header>
	);
}
