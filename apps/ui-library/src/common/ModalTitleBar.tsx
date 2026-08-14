import type { ReactNode } from "react";
import {
	TitleChrome,
	type TitleAction,
	type TitleActionGroup,
	type TitleSearch,
} from "./TitleChrome";

export type ModalTitleBarProps = {
	title: ReactNode;
	titleId?: string;
	details?: ReactNode;
	groups?: TitleActionGroup[];
	search?: TitleSearch;
	accept?: TitleAction;
	/** Non-action content such as a portal target. Use groups/accept for controls. */
	toolbar?: ReactNode;
	onClose?: () => void;
	closeDisabled?: boolean;
	closeLabel?: string;
	className?: string;
};

export function ModalTitleBar({
	title,
	titleId,
	details,
	groups = [],
	toolbar,
	accept,
	onClose,
	closeDisabled = false,
	closeLabel = "Close modal",
	className = "",
	search,
}: ModalTitleBarProps) {
	const resolvedGroups = [...groups];
	const terminals: TitleAction[] = [];
	if (accept) terminals.push(accept);
	terminals.push({
		id: "close",
		icon: <span aria-hidden="true">×</span>,
		ariaLabel: closeLabel,
		disabled: closeDisabled || !onClose,
		className: "ui-modal-title-close",
		onPress: onClose,
	});
	return (
		<header className={`ui-modal-titlebar ${className}`.trim()}>
			<div className="ui-modal-title-copy">
				<h2 id={titleId} className="ui-modal-title-heading">
					{title}
				</h2>
				{details && <div className="ui-modal-title-details">{details}</div>}
			</div>
			<span className="ui-modal-title-spacer" />
			<TitleChrome
				groups={resolvedGroups}
				search={
					search
						? {
								...search,
								ariaLabel: search.ariaLabel ?? titleSearchLabel(title),
								settingsTitle:
									search.settingsTitle ?? titleSettingsLabel(title),
							}
						: undefined
				}
				terminalActions={terminals}
				leadingTerminalContent={toolbar}
				groupClassName="ui-modal-title-tabs"
				searchClassName="ui-modal-title-search"
				terminalClassName="ui-modal-title-terminals"
			/>
		</header>
	);
}

function titleSearchLabel(title: ReactNode) {
	return typeof title === "string" ? `Search ${title}` : "Search";
}

function titleSettingsLabel(title: ReactNode) {
	return typeof title === "string"
		? `${title} search settings`
		: "Search settings";
}
