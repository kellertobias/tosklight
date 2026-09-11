import { SelectField } from "@tosklight/ui";
import { useEffect, useState } from "react";
import {
	documentSession,
	type NetworkInterface,
	type RendererSettings,
} from "./document/session";

type InterfaceField = "artNetInterface" | "sacnInterface";

const PROTOCOLS: ReadonlyArray<{ field: InterfaceField; label: string }> = [
	{ field: "artNetInterface", label: "Art-Net" },
	{ field: "sacnInterface", label: "sACN" },
];

const ALL = "";

/**
 * Which network interface this computer receives Art-Net and sACN on.
 *
 * The choice belongs to the computer, not the show: it is saved with the Visualizer's settings,
 * so the Visualizer and the Values tab listen on the same network, and a show taken to another
 * machine never carries this one's adapter names. An interface is chosen by name, so a renewed
 * address is still the same choice; one that is not connected receives nothing and says so,
 * rather than quietly falling back to every network.
 */
export function DmxInterfacesPanel({
	onError,
}: {
	onError: (reason: unknown) => void;
}) {
	const [settings, setSettings] = useState<RendererSettings | null>(null);
	const [interfaces, setInterfaces] = useState<NetworkInterface[] | null>(null);
	const [listError, setListError] = useState("");
	const [saving, setSaving] = useState(false);
	const [status, setStatus] = useState("");

	useEffect(() => {
		let current = true;
		documentSession
			.rendererSettings()
			.then((stored) => current && setSettings(stored))
			.catch(onError);
		return () => {
			current = false;
		};
	}, [onError]);

	// An adapter plugged in, or a cable connected, appears without reopening the tab.
	useEffect(() => {
		let current = true;
		const look = () =>
			void documentSession
				.networkInterfaces()
				.then((found) => {
					if (!current) return;
					setInterfaces(found);
					setListError("");
				})
				.catch((reason) => {
					if (!current) return;
					setInterfaces([]);
					setListError(String(reason));
				});
		look();
		const timer = window.setInterval(look, 5000);
		return () => {
			current = false;
			window.clearInterval(timer);
		};
	}, []);

	async function choose(field: InterfaceField, label: string, name: string) {
		if (!settings) return;
		setSaving(true);
		setStatus(`Saving the ${label} interface…`);
		try {
			const stored = await documentSession.saveRendererSettings({
				...settings,
				[field]: name === ALL ? null : name,
			});
			setSettings(stored);
			setStatus(
				name === ALL
					? `${label} is received on every interface.`
					: `${label} is received on ${name} only.`,
			);
		} catch (reason) {
			setStatus("");
			onError(reason);
		} finally {
			setSaving(false);
		}
	}

	const available = byName(interfaces ?? []);
	return (
		<section
			className="viz-live-inputs viz-dmx-interfaces"
			aria-labelledby="dmx-interfaces-title"
		>
			<header>
				<div>
					<h2 id="dmx-interfaces-title">Input Interfaces</h2>
					<p>
						The network this computer receives Art-Net and sACN on. Kept on this
						computer with the Visualizer’s settings and never stored in the show.
					</p>
				</div>
			</header>
			<div className="viz-dmx-interface-fields">
				{PROTOCOLS.map(({ field, label }) => {
					const chosen = settings?.[field] ?? null;
					const missing = chosen != null && !available.has(chosen);
					return (
						<div key={field} className="viz-dmx-interface-field">
							<SelectField
								label={`${label} interface`}
								value={chosen ?? ALL}
								disabled={!settings || interfaces == null || saving}
								onChange={(name) => void choose(field, label, name)}
								options={[
									{ value: ALL, label: "All interfaces" },
									...[...available].map(([name, addresses]) => ({
										value: name,
										label: interfaceLabel(name, addresses),
									})),
									...(missing && chosen
										? [{ value: chosen, label: `${chosen} · not connected` }]
										: []),
								]}
							/>
							{missing ? (
								<output className="viz-live-input-error" role="alert">
									{chosen} is not connected. No {label} is received until it
									returns, or until another interface is chosen.
								</output>
							) : null}
						</div>
					);
				})}
			</div>
			{listError ? (
				<output className="viz-live-input-error" role="alert">
					{listError}
				</output>
			) : null}
			{status ? <output className="viz-editor-status">{status}</output> : null}
		</section>
	);
}

/** Every interface once, with each IPv4 address it holds, in the order the machine lists them. */
function byName(interfaces: readonly NetworkInterface[]) {
	const named = new Map<string, NetworkInterface[]>();
	for (const candidate of interfaces) {
		named.set(candidate.name, [...(named.get(candidate.name) ?? []), candidate]);
	}
	return named;
}

function interfaceLabel(name: string, addresses: readonly NetworkInterface[]) {
	const listed = addresses.map((address) => address.address).join(", ");
	return addresses.every((address) => address.loopback)
		? `${name} · ${listed} (this computer only)`
		: `${name} · ${listed}`;
}
