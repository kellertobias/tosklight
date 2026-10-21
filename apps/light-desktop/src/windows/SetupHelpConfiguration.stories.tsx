import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo } from "react";
import {
	configuredExtensions,
	configuredMedia,
	configuredUsbDmx,
	documentationSession,
	installDocumentationAudioInput,
} from "../../../ui-library/storybook/fixtures/configuredSetup";
import { SetupHelpReady } from "../../../ui-library/storybook/providers/SetupHelpReady";
import { ConfigurationStateProvider } from "../features/configuration/ConfigurationState";
import { ConfigurationStore } from "../features/configuration/store";
import { MediaServersProvider } from "../features/mediaServers/MediaServersContext";
import { UsbDmxActionsProvider } from "../features/usbDmx/UsbDmxActions";
import {
	marketingConfiguration,
	marketingOutputRoutes,
	MarketingSetupWindow,
} from "./SetupWindow.stories";

const meta = {
	tags: ["autodocs"],
	title: "ToskLight/Windows/Configured Setup",
	parameters: { layout: "fullscreen" },
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

function ConfiguredSetup({
	subject,
}: {
	subject: "sound" | "extensions" | "matter" | "routes" | "usb";
}) {
	const store = useMemo(() => {
		const configuration = new ConfigurationStore();
		configuration.install({ ...marketingConfiguration, matter_enabled: true });
		return configuration;
	}, []);
	const usb = useMemo(() => {
		const snapshot =
			subject === "usb"
				? configuredUsbDmx
				: {
						document: { revision: 1, endpoints: [] },
						diagnostics: [],
						discovered_devices: [],
						replayed: false,
					};
		return {
			load: async () => snapshot,
			upsert: async () => snapshot,
			remove: async () => snapshot,
			resetMalformed: async () => snapshot,
		};
	}, [subject]);
	const routes =
		subject === "usb"
			? [
					{
						...marketingOutputRoutes[0],
						id: "front-usb-route",
						body: {
							...marketingOutputRoutes[0].body,
							logical_universe: 1,
							target: {
								kind: "usb_endpoint" as const,
								endpoint_id: "front-usb",
							},
						},
					},
				]
			: marketingOutputRoutes;
	return (
		<SetupHelpReady
			extensionStatus={configuredExtensions}
			session={documentationSession}
		>
			<ConfigurationStateProvider store={store}>
				<MediaServersProvider media={configuredMedia}>
					<UsbDmxActionsProvider actions={usb}>
						<MarketingSetupWindow
							initialSection={
								subject === "routes" || subject === "usb"
									? "outputs"
									: "network"
							}
							initialOutputsTab="routes"
							initialNetworkTab={
								subject === "sound"
									? "sound"
									: subject === "matter"
										? "bridges"
										: "control-server"
							}
							outputRoutes={routes}
						/>
					</UsbDmxActionsProvider>
				</MediaServersProvider>
			</ConfigurationStateProvider>
		</SetupHelpReady>
	);
}
export const SoundInput: Story = {
	beforeEach: installDocumentationAudioInput,
	render: () => <ConfiguredSetup subject="sound" />,
};
export const NativeExtensions: Story = {
	render: () => <ConfiguredSetup subject="extensions" />,
};
export const MatterBridge: Story = {
	render: () => <ConfiguredSetup subject="matter" />,
};
export const DmxRoutes: Story = {
	render: () => <ConfiguredSetup subject="routes" />,
};
export const UsbDmx: Story = {
	render: () => <ConfiguredSetup subject="usb" />,
};
