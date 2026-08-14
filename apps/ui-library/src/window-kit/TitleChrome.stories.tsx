import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
	Button,
	ModalTitleBar,
	type TitleActionGroup,
	type TitleDropdown,
} from "../common";
import { WindowHeader } from "./WindowKit";

const meta = {
	title: "ToskLight/Window System/Title Chrome",
	component: WindowHeader,
	tags: ["autodocs"],
	args: { title: "Title chrome" },
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof WindowHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

const Icon = ({ children }: { children: string }) => (
	<span aria-hidden="true">{children}</span>
);

function useChromeGroups(dropdown?: TitleDropdown) {
	const [mode, setMode] = useState("select");
	const groups: TitleActionGroup[] = [
		{
			id: "mode",
			kind: "tabs",
			activeId: mode,
			onActiveChange: setMode,
			actions: [
				{ id: "select", label: "Select", icon: <Icon>↖</Icon> },
				{ id: "navigate", label: "Navigate", icon: <Icon>✋</Icon> },
			],
		},
		{
			id: "edit",
			actions: [
				{
					id: "add",
					label: "Add",
					icon: <Icon>＋</Icon>,
					onPress: () => undefined,
					dropdown,
				},
			],
		},
	];
	return { groups, mode };
}

function WindowChromeExample() {
	const [query, setQuery] = useState("");
	const [status, setStatus] = useState("Select mode active");
	const { groups, mode } = useChromeGroups();
	return (
		<div>
			<WindowHeader
				title="Cues"
				groups={groups}
				search={{ value: query, onSearch: setQuery, ariaLabel: "Search cues" }}
				settings
				onSettings={() => setStatus("Settings opened")}
			/>
			<p aria-live="polite" style={{ padding: 16 }}>
				{status}; {mode} tab; filter “{query || "all cues"}”
			</p>
		</div>
	);
}

export const WindowChrome: Story = { render: () => <WindowChromeExample /> };

function ModalChromeExample() {
	const [query, setQuery] = useState("");
	const [accepted, setAccepted] = useState("Not applied");
	const { groups, mode } = useChromeGroups();
	return (
		<div style={{ padding: 16 }}>
			<ModalTitleBar
				title="Cue Properties"
				groups={groups}
				search={{
					value: query,
					onSearch: setQuery,
					ariaLabel: "Search properties",
				}}
				accept={{
					id: "accept",
					label: "Apply",
					variant: "success",
					onPress: () => setAccepted("Applied"),
				}}
				onClose={() => setAccepted("Closed")}
			/>
			<p aria-live="polite">
				{accepted}; {mode}; {query || "no filter"}
			</p>
		</div>
	);
}

export const ModalChrome: Story = { render: () => <ModalChromeExample /> };

function DropdownItemsExample() {
	const [follow, setFollow] = useState(false);
	const [status, setStatus] = useState("No menu action yet");
	const dropdown: TitleDropdown = {
		kind: "items",
		ariaLabel: "Add options",
		items: [
			{
				kind: "action",
				id: "cue",
				label: "Add Cue",
				onPress: () => setStatus("Cue added"),
			},
			{ kind: "divider", id: "divider" },
			{
				kind: "action",
				id: "disabled",
				label: "Unavailable",
				disabled: true,
				onPress: () => undefined,
			},
			{
				kind: "toggle",
				id: "follow",
				label: "Follow Preload",
				checked: follow,
				onChange: setFollow,
			},
		],
	};
	const { groups } = useChromeGroups(dropdown);
	return (
		<div>
			<WindowHeader
				title="Cues"
				groups={groups}
				settings
				onSettings={() => undefined}
			/>
			<p aria-live="polite" style={{ padding: 16 }}>
				{status}; Follow Preload {follow ? "on" : "off"}
			</p>
		</div>
	);
}

export const DropdownItems: Story = { render: () => <DropdownItemsExample /> };

function CustomDropdownExample() {
	const [status, setStatus] = useState("Custom panel closed");
	const dropdown: TitleDropdown = {
		kind: "content",
		ariaLabel: "Custom cue tools",
		render: ({ close }) => (
			<div style={{ display: "grid", gap: 8, padding: 8 }}>
				<b>Cue tools</b>
				<Button
					onClick={() => {
						setStatus("Custom close used");
						close();
					}}
				>
					Apply and close
				</Button>
			</div>
		),
	};
	const { groups } = useChromeGroups(dropdown);
	return (
		<div>
			<WindowHeader
				title="Cues"
				groups={groups}
				settings
				onSettings={() => undefined}
			/>
			<p aria-live="polite" style={{ padding: 16 }}>
				{status}
			</p>
		</div>
	);
}

export const CustomDropdownContent: Story = {
	render: () => <CustomDropdownExample />,
};

function ContentContractsExample() {
	const groups: TitleActionGroup[] = [
		{
			id: "content",
			actions: [
				{ id: "label", label: "Label only", onPress: () => undefined },
				{
					id: "icon",
					icon: <Icon>◎</Icon>,
					ariaLabel: "Icon only",
					onPress: () => undefined,
				},
				{
					id: "both",
					label: "Label and icon",
					icon: <Icon>＋</Icon>,
					onPress: () => undefined,
				},
			],
		},
	];
	return (
		<WindowHeader
			title="Content contract"
			groups={groups}
			settings
			onSettings={() => undefined}
		/>
	);
}

export const ContentContracts: Story = {
	render: () => <ContentContractsExample />,
};

function SearchSettingsExample() {
	const [query, setQuery] = useState("");
	return (
		<div style={{ display: "grid", gap: 16 }}>
			<WindowHeader
				title="Without settings"
				search={{ value: query, onSearch: setQuery }}
				settings
				onSettings={() => undefined}
			/>
			<WindowHeader
				title="With settings"
				search={{
					value: query,
					onSearch: setQuery,
					settings: (
						<div style={{ padding: 8 }}>Search cue name and number</div>
					),
				}}
				settings
				onSettings={() => undefined}
			/>
		</div>
	);
}

export const SearchWithAndWithoutSettings: Story = {
	render: () => <SearchSettingsExample />,
};

function ParityExample() {
	const [query, setQuery] = useState("");
	const { groups } = useChromeGroups();
	const chrome = {
		groups,
		search: { value: query, onSearch: setQuery, ariaLabel: "Shared search" },
	};
	return (
		<div style={{ display: "grid", gap: 24, padding: 16 }}>
			<section>
				<h3>Window</h3>
				<WindowHeader
					title="Cues"
					{...chrome}
					settings
					onSettings={() => undefined}
				/>
			</section>
			<section>
				<h3>Modal</h3>
				<ModalTitleBar
					title="Cues"
					{...chrome}
					accept={{ id: "accept", label: "Apply", onPress: () => undefined }}
					onClose={() => undefined}
				/>
			</section>
		</div>
	);
}

export const WindowModalParity: Story = { render: () => <ParityExample /> };
