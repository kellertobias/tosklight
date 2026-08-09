import { Button, SelectField, SwitchField, TextField } from "@tosklight/ui";
import { useEffect, useMemo, useState } from "react";
import type {
	UsbDmxDriverKind,
	UsbDmxEndpoint,
	UsbDmxEndpointSnapshot,
} from "../../api/client/deskManagement";
import { useUsbDmxActions } from "../../features/usbDmx/UsbDmxActions";

export interface UsbDmxEndpointsSetupProps {
	onSnapshot?: (snapshot: UsbDmxEndpointSnapshot | null) => void;
}

function installSnapshot(
	next: UsbDmxEndpointSnapshot,
	setSnapshot: (snapshot: UsbDmxEndpointSnapshot) => void,
	onSnapshot?: (snapshot: UsbDmxEndpointSnapshot | null) => void,
) {
	setSnapshot(next);
	onSnapshot?.(next);
}

function useUsbDmxSnapshot(
	actions: ReturnType<typeof useUsbDmxActions>,
	onSnapshot?: (snapshot: UsbDmxEndpointSnapshot | null) => void,
) {
	const [snapshot, setSnapshot] = useState<UsbDmxEndpointSnapshot | null>(null);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState("");
	const install = (next: UsbDmxEndpointSnapshot) =>
		installSnapshot(next, setSnapshot, onSnapshot);
	const reload = async () => {
		if (!actions) return;
		setBusy(true);
		setError("");
		try {
			install(await actions.load());
		} catch (cause) {
			setError(cause instanceof Error ? cause.message : String(cause));
		} finally {
			setBusy(false);
		}
	};
	useEffect(() => {
		void reload();
		// The provider is stable for one connected desk session.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [actions]);
	return { snapshot, busy, setBusy, error, setError, install, reload };
}

function EndpointList({
	snapshot,
	busy,
	remove,
}: {
	snapshot: UsbDmxEndpointSnapshot | null;
	busy: boolean;
	remove: (id: string) => Promise<void>;
}) {
	return (
		<div className="setup-list usb-dmx-endpoint-list">
			{snapshot?.document.endpoints.map((endpoint) => {
				const diagnostic = snapshot.diagnostics.find(
					(item) => item.endpoint_id === endpoint.endpoint_id,
				);
				return (
					<article key={endpoint.endpoint_id}>
						<span>
							<b>{endpoint.endpoint_id}</b>
							<small>
								{endpoint.driver === "open_dmx"
									? "Open DMX"
									: "ENTTEC USB Pro v1.44"}
								{" · "}
								{endpoint.identity.product ?? "USB serial device"}
								{" · "}
								{diagnostic?.message ?? "Waiting for diagnostics"}
							</small>
						</span>
						<span
							className={
								diagnostic?.code === "ready"
									? "route-enabled"
									: "route-disabled"
							}
						>
							{diagnostic?.code ??
								(endpoint.enabled ? "Configured" : "Disabled")}
						</span>
						<Button
							disabled={busy}
							onClick={() => void remove(endpoint.endpoint_id)}
						>
							Remove endpoint
						</Button>
					</article>
				);
			})}
			{snapshot && !snapshot.document.endpoints.length && (
				<p className="empty-window-message">No USB DMX endpoint is claimed.</p>
			)}
		</div>
	);
}

function ClaimForm(props: {
	unclaimed: NonNullable<UsbDmxEndpointSnapshot["discovered_devices"]>;
	selectedPort: string;
	setSelectedPort: (value: string) => void;
	endpointId: string;
	setEndpointId: (value: string) => void;
	driver: UsbDmxDriverKind;
	setDriver: (value: UsbDmxDriverKind) => void;
	enabled: boolean;
	setEnabled: (value: boolean) => void;
	busy: boolean;
	selected: unknown;
	claim: () => Promise<void>;
}) {
	const {
		unclaimed,
		selectedPort,
		setSelectedPort,
		endpointId,
		setEndpointId,
	} = props;
	return (
		<div className="usb-dmx-claim-form">
			<SelectField
				label="Discovered device"
				value={selectedPort}
				onChange={(value) => {
					setSelectedPort(value);
					const device = unclaimed.find(
						(candidate) => candidate.port_name === value,
					);
					if (device && !endpointId)
						setEndpointId(
							(device.identity.product ?? "usb-dmx")
								.toLowerCase()
								.replace(/[^a-z0-9]+/gu, "-")
								.replace(/^-|-$/gu, ""),
						);
				}}
				options={[
					{ value: "", label: "Choose a USB serial device" },
					...unclaimed.map((device) => ({
						value: device.port_name,
						label: `${device.identity.product ?? "USB serial device"} · ${device.port_name}`,
					})),
				]}
			/>
			<TextField
				label="Endpoint ID"
				value={endpointId}
				onChange={(event) => setEndpointId(event.target.value)}
			/>
			<SelectField
				label="Driver"
				value={props.driver}
				onChange={props.setDriver}
				options={[
					{ value: "enttec_usb_pro_v144", label: "ENTTEC USB Pro v1.44" },
					{ value: "open_dmx", label: "Open DMX (FTDI)" },
				]}
			/>
			<SwitchField
				label="Endpoint state"
				offLabel="Disabled"
				onLabel="Enabled"
				checked={props.enabled}
				onChange={(event) => props.setEnabled(event.target.checked)}
			/>
			<Button
				variant="primary"
				disabled={props.busy || !props.selected || !endpointId.trim()}
				onClick={() => void props.claim()}
			>
				Claim endpoint
			</Button>
		</div>
	);
}

export function UsbDmxEndpointsSetup({
	onSnapshot,
}: UsbDmxEndpointsSetupProps) {
	const actions = useUsbDmxActions();
	const { snapshot, busy, setBusy, error, setError, install, reload } =
		useUsbDmxSnapshot(actions, onSnapshot);
	const [selectedPort, setSelectedPort] = useState("");
	const [endpointId, setEndpointId] = useState("");
	const [driver, setDriver] = useState<UsbDmxDriverKind>("enttec_usb_pro_v144");
	const [enabled, setEnabled] = useState(true);

	const unclaimed = useMemo(() => {
		const claimed = new Set(
			snapshot?.document.endpoints.map((endpoint) =>
				endpoint.identity.usb_serial
					? `serial:${endpoint.identity.usb_serial}`
					: `port:${endpoint.identity.port_topology_hint}`,
			),
		);
		return (snapshot?.discovered_devices ?? []).filter(
			(device) =>
				!claimed.has(
					device.identity.usb_serial
						? `serial:${device.identity.usb_serial}`
						: `port:${device.port_name}`,
				),
		);
	}, [snapshot]);
	const selected = unclaimed.find(
		(device) => device.port_name === selectedPort,
	);
	const claim = async () => {
		if (!actions || !snapshot || !selected || !endpointId.trim()) return;
		const endpoint: UsbDmxEndpoint = {
			endpoint_id: endpointId.trim(),
			driver,
			identity: {
				...selected.identity,
				port_topology_hint: selected.identity.usb_serial
					? null
					: selected.port_name,
			},
			enabled,
		};
		setBusy(true);
		setError("");
		try {
			install(await actions.upsert(snapshot.document.revision, endpoint));
			setSelectedPort("");
			setEndpointId("");
		} catch (cause) {
			setError(cause instanceof Error ? cause.message : String(cause));
		} finally {
			setBusy(false);
		}
	};
	const remove = async (id: string) => {
		if (!actions || !snapshot) return;
		setBusy(true);
		setError("");
		try {
			install(await actions.remove(snapshot.document.revision, id));
		} catch (cause) {
			setError(cause instanceof Error ? cause.message : String(cause));
		} finally {
			setBusy(false);
		}
	};
	const resetMalformed = async () => {
		if (!actions || !snapshot) return;
		setBusy(true);
		setError("");
		try {
			install(await actions.resetMalformed(snapshot.document.revision));
		} catch (cause) {
			setError(cause instanceof Error ? cause.message : String(cause));
		} finally {
			setBusy(false);
		}
	};

	return (
		<section className="usb-dmx-endpoints-setup" aria-label="USB DMX endpoints">
			<header>
				<div>
					<h3>USB DMX endpoints</h3>
					<small>
						Claim one exact USB identity before assigning a show output route.
					</small>
				</div>
				<Button disabled={busy || !actions} onClick={() => void reload()}>
					Scan USB devices
				</Button>
			</header>
			{snapshot?.configuration_error && (
				<div className="delete-confirm" role="alert">
					<span>
						Stored USB DMX settings are malformed:{" "}
						{snapshot.configuration_error}
					</span>
					<Button disabled={busy} onClick={() => void resetMalformed()}>
						Reset USB DMX settings
					</Button>
				</div>
			)}
			{snapshot?.discovery_error && (
				<p className="ui-field-error" role="alert">
					USB discovery failed: {snapshot.discovery_error}
				</p>
			)}
			<EndpointList snapshot={snapshot} busy={busy} remove={remove} />
			{unclaimed.length > 0 && (
				<ClaimForm
					{...{
						unclaimed,
						selectedPort,
						setSelectedPort,
						endpointId,
						setEndpointId,
						driver,
						setDriver,
						enabled,
						setEnabled,
						busy,
						selected,
						claim,
					}}
				/>
			)}
			{error && (
				<p className="ui-field-error" role="alert">
					{error}
				</p>
			)}
		</section>
	);
}
