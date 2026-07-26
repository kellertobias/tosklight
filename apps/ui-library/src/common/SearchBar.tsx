import { useRef, useState } from "react";
import { ModalLayer } from "../modals/ModalStack";
import { ModalTitleBar } from "./ModalTitleBar";
import {
	Button,
	FormLayout,
	SelectField,
	SwitchField,
	TextField,
	TextInput,
} from "./controls";

export type SearchSetting =
	| {
			kind: "select";
			id: string;
			label: string;
			value: string;
			options: readonly { value: string; label: string }[];
			description?: string;
	  }
	| {
			kind: "toggle";
			id: string;
			label: string;
			value: boolean;
			description?: string;
	  }
	| {
			kind: "text";
			id: string;
			label: string;
			value: string;
			placeholder?: string;
			description?: string;
			keyboardInitiallyOpen?: boolean;
	  };

export interface SearchFeatureConfiguration {
	value: string;
	placeholder?: string;
	ariaLabel?: string;
	settingsTitle?: string;
	keyboardInitiallyOpen?: boolean;
	settingsInitiallyOpen?: boolean;
	settings?: readonly SearchSetting[];
	onSettingChange?: (id: string, value: string | boolean) => void;
	onClearSettings?: () => void;
}

export type SearchFeatureProps =
	| { onSearch?: undefined; search?: never }
	| { onSearch: (value: string) => void; search: SearchFeatureConfiguration };

export function SearchBar({
	value,
	onChange,
	settings = [],
	onSettingChange,
	onClearSettings,
	placeholder = "Search",
	ariaLabel = "Search",
	settingsTitle = "Search settings",
	keyboardInitiallyOpen = false,
	settingsInitiallyOpen = false,
}: SearchFeatureConfiguration & { onChange: (value: string) => void }) {
	const [open, setOpen] = useState(settingsInitiallyOpen);
	const optionsButton = useRef<HTMLButtonElement>(null);
	const hasOptions = settings.length > 0;
	const closeOptions = () => {
		setOpen(false);
	};
	const filterDialog = (
		<ModalLayer
			ariaLabel={settingsTitle}
			className="search-options-layer"
			dialogClassName="search-filter-modal"
			onClose={closeOptions}
		>
			<ModalTitleBar
				title={settingsTitle}
				closeLabel="Close search settings"
				onClose={closeOptions}
			/>
			<div className="search-settings-body">
				<FormLayout
					className="search-settings-fields"
					labelPlacement="side"
					labelWidth={150}
				>
					{settings.map((setting) =>
						setting.kind === "select" ? (
							<SelectField
								key={setting.id}
								label={setting.label}
								description={setting.description}
								value={setting.value}
								options={[...setting.options]}
								onChange={(next) => onSettingChange?.(setting.id, next)}
							/>
						) : setting.kind === "toggle" ? (
							<SwitchField
								key={setting.id}
								label={setting.label}
								description={setting.description}
								checked={setting.value}
								onChange={(event) =>
									onSettingChange?.(setting.id, event.target.checked)
								}
							/>
						) : (
							<TextField
								key={setting.id}
								label={setting.label}
								description={setting.description}
								value={setting.value}
								placeholder={setting.placeholder}
								openKeyboardInitially={setting.keyboardInitiallyOpen}
								onValueChange={(next) =>
									onSettingChange?.(setting.id, next)
								}
							/>
						),
					)}
				</FormLayout>
			</div>
			{onClearSettings && (
				<footer className="search-settings-actions">
					<Button
						onClick={onClearSettings}
					>
						Clear settings
					</Button>
				</footer>
			)}
		</ModalLayer>
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
						aria-label="Search settings"
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
					openKeyboardInitially={keyboardInitiallyOpen}
					aria-label={ariaLabel}
					value={value}
					placeholder={placeholder}
					onChange={(event) => onChange(event.target.value)}
				/>
			</div>
			{open && filterDialog}
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
