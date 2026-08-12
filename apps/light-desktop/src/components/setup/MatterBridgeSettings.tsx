import { Button, SwitchField } from "@tosklight/ui";
import { useConfigurationActions } from "../../features/configuration/ConfigurationActionsProvider";
import {
	useDeskConfiguration,
	useMatterEnabled,
} from "../../features/configuration/ConfigurationState";
import { useMediaServers } from "../../features/mediaServers/MediaServersContext";

export function MatterBridgeSettings() {
	const matter = useMediaServers()?.matter ?? null;
	const configuration = useDeskConfiguration();
	const configurationActions = useConfigurationActions();
	const enabled = useMatterEnabled();
	const dimmableCount =
		matter?.lights.filter((light) => light.kind === "dimmable").length ?? 0;
	const colorCount =
		matter?.lights.filter((light) => light.kind === "color").length ?? 0;
	const toggleMatter = (enabled: boolean) => {
		if (!configuration) return;
		void configurationActions?.saveConfiguration({
			...configuration,
			matter_enabled: enabled,
		});
	};

	return (
		<div className="matter-desk-settings" aria-label="Matter bridge settings">
			<small>Desk installation · shared across shows and Desktops</small>
			<SwitchField
				label="Matter server"
				offLabel="Disabled"
				onLabel="Enabled"
				checked={enabled}
				onChange={(event) => toggleMatter(event.target.checked)}
			/>
			<p>
				{!enabled
					? "Disabled. No Matter lights are advertised."
					: matter?.transport === "running"
						? `${matter.lights.length} assigned playback${matter.lights.length === 1 ? "" : "s"} exposed: ${dimmableCount} dimmable, ${colorCount} color.`
						: (matter?.limitation ?? "Starting Matter networking…")}
			</p>
			{enabled && matter && matter.lights.length > 0 && (
				<ul className="matter-playback-list" aria-label="Exposed Matter playbacks">
					{matter.lights.map((light) => (
						<li key={light.endpoint_id}>
							<span>{light.name}</span>
							<b>
								{light.kind === "color"
									? `Color${light.color_active ? " · Group color active" : ""}`
									: "Dimmable"}
							</b>
						</li>
					))}
				</ul>
			)}
			{matter?.commissionable && matter.pairing && (
				<div className="matter-pairing">
					<b>Ready to commission</b>
					<span>Manual pairing code</span>
					<code>{matter.pairing.manual_code}</code>
					<Button
						onClick={() =>
							void navigator.clipboard?.writeText(
								matter.pairing?.manual_code ?? "",
							)
						}
					>
						Copy pairing code
					</Button>
					<details>
						<summary>QR payload</summary>
						<code>{matter.pairing.qr_code}</code>
					</details>
				</div>
			)}
			{matter?.commissioned && (
				<small>
					Commissioned on the local Matter fabric. Playback changes and
					controller writes are synchronized in both directions.
				</small>
			)}
			{enabled && (
				<small>
					Only assigned Cuelist, Dynamic, and Group Master playbacks are
					advertised. Group Masters include color control; Cuelists and Dynamics
					are dimmable. Empty slots, unassigned pool entries, and other playback
					targets are omitted.
				</small>
			)}
		</div>
	);
}
