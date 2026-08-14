import {
	type CSSProperties,
	type PointerEvent,
	type ReactNode,
	useEffect,
	useRef,
	useState,
} from "react";
import { createPortal } from "react-dom";
import { Button, type ButtonVariant } from "./controls";
import {
	SearchBar,
	type SearchFeatureConfiguration,
	type SearchSetting,
} from "./SearchBar";

export type TitleActionContent =
	| { label: ReactNode; icon?: ReactNode; ariaLabel?: string }
	| { label?: never; icon: ReactNode; ariaLabel: string };

type TitleActionBase = TitleActionContent & {
	id: string;
	disabled?: boolean;
	variant?: ButtonVariant;
	active?: boolean;
	loading?: boolean;
	onPress?: () => void;
	onLongPress?: () => void;
	className?: string;
	type?: "button" | "submit";
};

export type TitleAction =
	| (TitleActionBase & { kind?: "button"; dropdown?: TitleDropdown })
	| (TitleActionBase & { kind: "dropdown"; dropdown: TitleDropdown });

export type TitleActionGroup =
	| {
			id: string;
			kind?: "buttons";
			actions: TitleAction[];
			dropdown?: TitleDropdown;
	  }
	| {
			id: string;
			kind: "tabs";
			activeId: string;
			onActiveChange: (id: string) => void;
			actions: Array<
				TitleActionContent & {
					id: string;
					disabled?: boolean;
					className?: string;
				}
			>;
			dropdown?: TitleDropdown;
	  };

export type TitleDropdown =
	| { kind: "items"; items: TitleDropdownItem[]; ariaLabel?: string }
	| {
			kind: "content";
			ariaLabel: string;
			render: (api: { close(): void }) => ReactNode;
	  };

export type TitleDropdownItem =
	| {
			kind: "action";
			id: string;
			label: ReactNode;
			disabled?: boolean;
			onPress(): void;
	  }
	| {
			kind: "toggle";
			id: string;
			label: ReactNode;
			checked: boolean;
			disabled?: boolean;
			onChange(checked: boolean): void;
	  }
	| { kind: "divider"; id: string };

export interface TitleSearch
	extends Omit<SearchFeatureConfiguration, "settings"> {
	onSearch(value: string): void;
	settings?: ReactNode;
	settingsConfiguration?: readonly SearchSetting[];
}

export interface TitleChromeProps {
	groups?: TitleActionGroup[];
	search?: TitleSearch;
	terminalActions: TitleAction[];
	/** Compatibility slot while application-owned modal actions migrate. */
	leadingTerminalContent?: ReactNode;
	className?: string;
	searchClassName?: string;
	groupClassName?: string;
	terminalClassName?: string;
}

export function TitleChrome({
	groups = [],
	search,
	terminalActions,
	leadingTerminalContent,
	className = "",
	searchClassName = "",
	groupClassName = "",
	terminalClassName = "",
}: TitleChromeProps) {
	const populatedGroups = groups.filter((group) => group.actions.length > 0);
	return (
		<div className={`ui-title-chrome ${className}`.trim()}>
			<div className="ui-title-chrome-groups">
				{populatedGroups.map((group) => (
					<TitleGroup key={group.id} group={group} className={groupClassName} />
				))}
			</div>
			{search && (
				<div className={`ui-title-chrome-search ${searchClassName}`.trim()}>
					<TitleSearchControl search={search} />
				</div>
			)}
			{(leadingTerminalContent != null || terminalActions.length > 0) && (
				<div
					className={`ui-title-chrome-terminals ${terminalClassName}`.trim()}
				>
					{leadingTerminalContent != null && (
						<div className="ui-modal-title-actions">
							{leadingTerminalContent}
						</div>
					)}
					{terminalActions.map((action) => (
						<TitleActionControl key={action.id} action={action} />
					))}
				</div>
			)}
		</div>
	);
}

function TitleGroup({
	group,
	className,
}: {
	group: TitleActionGroup;
	className: string;
}) {
	if (group.kind === "tabs") {
		validateTabGroup(group);
		return (
			<div
				className={`ui-title-chrome-group ui-title-chrome-tabs ${className}`.trim()}
				role="tablist"
			>
				{group.actions.map((action) => (
					<TitleActionControl
						key={action.id}
						action={{
							...action,
							active: action.id === group.activeId,
							onPress: () => group.onActiveChange(action.id),
						}}
						role="tab"
						aria-selected={action.id === group.activeId}
					/>
				))}
				{group.dropdown && (
					<TitleDropdownTrigger
						dropdown={group.dropdown}
						ariaLabel={`${group.id} options`}
					/>
				)}
			</div>
		);
	}
	return (
		<div className={`ui-title-chrome-group ${className}`.trim()}>
			{group.actions.map((action) => (
				<TitleActionControl key={action.id} action={action} />
			))}
			{group.dropdown && (
				<TitleDropdownTrigger
					dropdown={group.dropdown}
					ariaLabel={`${group.id} options`}
				/>
			)}
		</div>
	);
}

function TitleActionControl({
	action,
	...buttonProps
}: {
	action: TitleAction;
	role?: string;
	"aria-selected"?: boolean;
}) {
	validateActionContent(action);
	const holdTimer = useRef<number | null>(null);
	const held = useRef(false);
	const clearHold = () => {
		if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
		holdTimer.current = null;
	};
	const content = action.label ?? action.icon;
	const iconOnly = action.label == null;
	if (action.kind === "dropdown") {
		return (
			<TitleDropdownTrigger
				dropdown={action.dropdown}
				ariaLabel={action.ariaLabel ?? String(action.label)}
				label={content}
				disabled={action.disabled}
				className={action.className}
				iconOnly={iconOnly}
				variant={action.variant}
			/>
		);
	}
	return (
		<span
			className={`ui-title-chrome-action ${action.dropdown ? "has-dropdown" : ""}`}
		>
			<Button
				{...buttonProps}
				aria-label={action.ariaLabel}
				disabled={action.disabled}
				variant={action.variant}
				active={action.active}
				loading={action.loading}
				iconOnly={iconOnly}
				icon={action.label != null ? action.icon : undefined}
				className={action.className}
				type={action.type}
				onPointerDown={
					action.onLongPress
						? () => {
								held.current = false;
								clearHold();
								holdTimer.current = window.setTimeout(() => {
									held.current = true;
									action.onLongPress?.();
								}, 650);
							}
						: undefined
				}
				onPointerUp={action.onLongPress ? clearHold : undefined}
				onPointerCancel={action.onLongPress ? clearHold : undefined}
				onPointerLeave={action.onLongPress ? clearHold : undefined}
				onClick={() => {
					if (held.current) {
						held.current = false;
						return;
					}
					action.onPress?.();
				}}
			>
				{action.label != null ? action.label : action.icon}
			</Button>
			{action.dropdown && (
				<TitleDropdownTrigger
					dropdown={action.dropdown}
					ariaLabel={`${action.ariaLabel ?? String(action.label)} options`}
				/>
			)}
		</span>
	);
}

function TitleSearchControl({ search }: { search: TitleSearch }) {
	const { onSearch, settings, settingsConfiguration, ...configuration } = search;
	const settingsLabel = search.settingsTitle ?? "Search settings";
	return (
		<div className="ui-title-chrome-search-control">
			<SearchBar
				{...configuration}
				settings={settingsConfiguration}
				onChange={onSearch}
			/>
			{search.settings != null && (
				<TitleDropdownTrigger
					ariaLabel={settingsLabel}
					label="⚙"
					iconOnly
					dropdown={{
						kind: "content",
						ariaLabel: settingsLabel,
						render: () => search.settings,
					}}
				/>
			)}
		</div>
	);
}

function TitleDropdownTrigger({
	dropdown,
	ariaLabel,
	label = "⌄",
	disabled,
	className = "",
	iconOnly = true,
	variant,
}: {
	dropdown: TitleDropdown;
	ariaLabel: string;
	label?: ReactNode;
	disabled?: boolean;
	className?: string;
	iconOnly?: boolean;
	variant?: ButtonVariant;
}) {
	const trigger = useRef<HTMLButtonElement>(null);
	const [anchor, setAnchor] = useState<DOMRect | null>(null);
	const close = () => {
		setAnchor(null);
		requestAnimationFrame(() => trigger.current?.focus());
	};
	useEffect(() => {
		if (!anchor) return;
		const onKeyDown = (event: globalThis.KeyboardEvent) => {
			if (event.key === "Escape") close();
		};
		window.addEventListener("keydown", onKeyDown);
		return () => window.removeEventListener("keydown", onKeyDown);
	}, [anchor]);
	const style = anchor
		? ({
				top: Math.min(anchor.bottom + 3, Math.max(3, window.innerHeight - 320)),
				left: Math.max(3, Math.min(anchor.left, window.innerWidth - 260)),
			} satisfies CSSProperties)
		: undefined;
	return (
		<>
			<Button
				ref={trigger}
				variant={variant}
				iconOnly={iconOnly}
				disabled={disabled}
				className={`ui-title-chrome-dropdown-trigger ${className}`.trim()}
				aria-label={ariaLabel}
				aria-haspopup="menu"
				aria-expanded={Boolean(anchor)}
				onClick={(event) => {
					const nextAnchor = event.currentTarget.getBoundingClientRect();
					setAnchor((current) => (current ? null : nextAnchor));
				}}
			>
				{label}
			</Button>
			{anchor &&
				createPortal(
					<div
						className="ui-title-chrome-dropdown-layer"
						onPointerDown={(event: PointerEvent<HTMLDivElement>) => {
							if (event.target === event.currentTarget) close();
						}}
					>
						<div
							className="ui-title-chrome-dropdown"
							role="menu"
							aria-label={dropdown.ariaLabel ?? ariaLabel}
							style={style}
						>
							{dropdown.kind === "items"
								? dropdown.items.map((item) =>
										item.kind === "divider" ? (
											<span
												key={item.id}
												className="ui-title-chrome-dropdown-divider"
												role="separator"
											/>
										) : (
											<Button
												key={item.id}
												role={
													item.kind === "toggle"
														? "menuitemcheckbox"
														: "menuitem"
												}
												aria-checked={
													item.kind === "toggle" ? item.checked : undefined
												}
												disabled={item.disabled}
												contentAlign="left"
												onClick={() => {
													if (item.kind === "toggle")
														item.onChange(!item.checked);
													else {
														item.onPress();
														close();
													}
												}}
											>
												{item.kind === "toggle" && (
													<span aria-hidden="true">
														{item.checked ? "✓" : ""}
													</span>
												)}
												{item.label}
											</Button>
										),
									)
								: dropdown.render({ close })}
						</div>
					</div>,
					document.body,
				)}
		</>
	);
}

export function validateTitleAction(action: TitleActionContent) {
	validateActionContent(action);
}

function validateActionContent(action: TitleActionContent) {
	if (action.label == null && action.icon == null)
		throw new Error("Title actions require a label, an icon, or both");
	if (action.label == null && !action.ariaLabel)
		throw new Error("Icon-only title actions require ariaLabel");
}

function validateTabGroup(group: Extract<TitleActionGroup, { kind: "tabs" }>) {
	if (!group.actions.some((action) => action.id === group.activeId))
		throw new Error(`Title tab group ${group.id} requires one valid activeId`);
}
