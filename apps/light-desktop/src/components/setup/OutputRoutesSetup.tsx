import {
	Button,
	FormLayout,
	ModalPortal,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useMemo, useState } from "react";
import type {
	UsbDmxDriverKind,
	UsbDmxEndpoint,
} from "../../api/client/deskManagement";
import type { OutputRoute, VersionedObject } from "../../api/types";
import {
	type DiscoveredUsbDmxDevice,
	recommendedUsbDmxDriver,
	usbDmxDeviceLabel,
} from "./UsbDmxEndpointsSetup";

interface RouteDraft {
	id: string;
	revision: number;
	body: OutputRoute;
	logicalUniverse: string;
	destinationUniverse: string;
	usbDevicePort: string | null;
	usbDriver: UsbDmxDriverKind;
}

export interface OutputRoutesSetupProps {
	routes: VersionedObject<OutputRoute>[];
	onSave: (
		id: string,
		route: OutputRoute,
		revision: number,
	) => Promise<boolean>;
	onCreateRange: (range: OutputRouteRangeIntent) => Promise<boolean>;
	onDelete: (id: string, revision: number) => Promise<boolean>;
	outputBindIp?: string;
	usbEndpoints?: UsbDmxEndpoint[];
	usbDevices?: DiscoveredUsbDmxDevice[];
	usbBusy?: boolean;
	usbError?: string;
	onScanUsbDevices?: () => Promise<void>;
	onProvisionUsbDevice?: (
		device: DiscoveredUsbDmxDevice,
		driver: UsbDmxDriverKind,
	) => Promise<UsbDmxEndpoint | null>;
}

export interface OutputRouteRangeIntent {
	logical_start: number;
	logical_end: number;
	destination_start: number;
	destination_end: number;
	route: Omit<OutputRoute, "logical_universe" | "destination_universe">;
}

function newRoute(device?: DiscoveredUsbDmxDevice): RouteDraft {
	const recommendedDriver = device && recommendedUsbDmxDriver(device);
	return {
		id: `route-${crypto.randomUUID()}`,
		revision: 0,
		logicalUniverse: "1",
		destinationUniverse: "1",
		usbDevicePort: device?.port_name ?? null,
		usbDriver: recommendedDriver ?? "enttec_usb_pro_v144",
		body: {
			protocol: "art_net",
			logical_universe: 1,
			destination_universe: 1,
			delivery_mode: "broadcast",
			destination: null,
			enabled: true,
			minimum_slots: 128,
			...(device
				? { target: { kind: "usb_endpoint" as const, endpoint_id: "" } }
				: {}),
		},
	};
}

export function parseUniverseExpression(value: string): number[] | null {
	const normalized = value.trim().replace(/\s+/gu, " ");
	const match = normalized.match(/^(\d+)(?:\s+THRU\s+(\d+))?$/iu);
	if (!match) return null;
	const first = Number(match[1]);
	const last = Number(match[2] ?? match[1]);
	if (!Number.isSafeInteger(first) || !Number.isSafeInteger(last)) return null;
	const direction = first <= last ? 1 : -1;
	return Array.from(
		{ length: Math.abs(last - first) + 1 },
		(_, index) => first + index * direction,
	);
}

function isIpv4(value: string): boolean {
	const parts = value.split(".");
	return (
		parts.length === 4 &&
		parts.every((part) => /^\d{1,3}$/.test(part) && Number(part) <= 255)
	);
}

function validate(route: OutputRoute, outputBindIp?: string): string {
	if (
		!Number.isInteger(route.logical_universe) ||
		route.logical_universe < 1 ||
		route.logical_universe > 65_535
	)
		return "Logical universe must be a whole number from 1 to 65535.";
	if (route.target?.kind === "usb_endpoint") {
		if (!route.target.endpoint_id.trim())
			return "Choose a USB DMX device.";
		return "";
	}
	const maximumUniverse = route.protocol === "art_net" ? 32_767 : 63_999;
	if (
		!Number.isInteger(route.destination_universe) ||
		route.destination_universe < 1 ||
		route.destination_universe > maximumUniverse
	)
		return `${route.protocol === "art_net" ? "Art-Net" : "sACN"} destination universe must be a whole number from 1 to ${maximumUniverse}.`;
	if (
		!Number.isInteger(route.minimum_slots) ||
		route.minimum_slots < 1 ||
		route.minimum_slots > 512
	)
		return "Minimum universe size must be a whole number from 1 to 512.";
	if (route.protocol === "art_net" && route.delivery_mode === "multicast")
		return "Art-Net supports Broadcast or Unicast delivery.";
	if (route.protocol === "sacn" && route.delivery_mode === "broadcast")
		return "sACN supports Multicast or Unicast delivery.";
	if (outputBindIp && !isIpv4(outputBindIp))
		return "The output bind address must be an available IPv4 interface before this route can be saved.";
	if (route.delivery_mode === "unicast") {
		const destination = route.destination?.trim() ?? "";
		const separator = destination.lastIndexOf(":");
		const address = destination.slice(0, separator);
		const port = Number(destination.slice(separator + 1));
		if (
			separator < 0 ||
			!isIpv4(address) ||
			!Number.isInteger(port) ||
			port < 1 ||
			port > 65_535
		)
			return "Unicast delivery requires an IPv4 destination and port, for example 10.0.0.20:6454.";
	}
	return "";
}

function modeLabel(route: OutputRoute): string {
	if (route.target?.kind === "usb_endpoint") return "USB DMX";
	if (route.protocol === "art_net")
		return route.delivery_mode === "unicast"
			? "Art-Net Unicast"
			: "Art-Net Broadcast";
	return route.delivery_mode === "unicast" ? "sACN Unicast" : "sACN Multicast";
}

function endpointLabel(endpoint: UsbDmxEndpoint | undefined) {
	if (!endpoint) return "USB DMX device";
	const product = endpoint.identity.product?.trim() || "USB DMX device";
	const serial = endpoint.identity.usb_serial?.trim();
	return serial ? `${product} · USB serial ${serial}` : product;
}

function routeUsbEndpoint(
	endpoints: readonly UsbDmxEndpoint[],
	route: OutputRoute,
) {
	const target = route.target;
	return target?.kind === "usb_endpoint"
		? endpoints.find((endpoint) => endpoint.endpoint_id === target.endpoint_id)
		: undefined;
}

export function OutputRoutesSetup({
	routes,
	onSave,
	onCreateRange,
	onDelete,
	outputBindIp,
	usbEndpoints = [],
	usbDevices = [],
	usbBusy = false,
	usbError = "",
	onScanUsbDevices,
	onProvisionUsbDevice,
}: OutputRoutesSetupProps) {
	const [draft, setDraft] = useState<RouteDraft | null>(null);
	const [error, setError] = useState("");
	const [busy, setBusy] = useState(false);
	const [confirmDelete, setConfirmDelete] = useState(false);
	const ordered = useMemo(
		() =>
			[...routes].sort(
				(left, right) =>
					left.body.logical_universe - right.body.logical_universe ||
					left.body.protocol.localeCompare(right.body.protocol) ||
					left.id.localeCompare(right.id),
			),
		[routes],
	);
	const edit = (route: VersionedObject<OutputRoute>) => {
		setError("");
		setConfirmDelete(false);
		setDraft({
			id: route.id,
			revision: route.revision,
			body: { ...route.body },
			logicalUniverse: String(route.body.logical_universe),
			destinationUniverse: String(route.body.destination_universe),
			usbDevicePort: null,
			usbDriver:
				routeUsbEndpoint(usbEndpoints, route.body)?.driver ??
				"enttec_usb_pro_v144",
		});
	};
	const close = () => {
		setDraft(null);
		setError("");
		setConfirmDelete(false);
	};
	const save = async () => {
		if (!draft) return;
		let body = draft.body;
		const logicalUniverses = parseUniverseExpression(draft.logicalUniverse);
		const usbTarget = body.target?.kind === "usb_endpoint";
		const destinationUniverses = usbTarget
			? logicalUniverses
			: parseUniverseExpression(draft.destinationUniverse);
		if (!logicalUniverses || !destinationUniverses) {
			setError("Universe values must be a number or a range such as 1 THRU 8.");
			return;
		}
		if (usbTarget && logicalUniverses.length !== 1) {
			setError("A USB DMX device accepts one logical universe per route.");
			return;
		}
		if (
			draft.revision > 0 &&
			(logicalUniverses.length > 1 || destinationUniverses.length > 1)
		) {
			setError(
				"Ranges create routes and are available only while adding a route.",
			);
			return;
		}
		if (logicalUniverses.length !== destinationUniverses.length) {
			setError(
				"Logical and destination universe ranges must contain the same number of universes.",
			);
			return;
		}
		const logicalEnd = logicalUniverses[logicalUniverses.length - 1];
		const destinationEnd =
			destinationUniverses[destinationUniverses.length - 1];
		if (
			logicalUniverses[0] > logicalEnd ||
			destinationUniverses[0] > destinationEnd
		) {
			setError("Universe ranges must be entered in ascending order.");
			return;
		}
		if (logicalUniverses.length > 128) {
			setError("One range can create at most 128 output routes.");
			return;
		}
		if (body.target?.kind === "usb_endpoint" && draft.usbDevicePort) {
			const device = usbDevices.find(
				(candidate) => candidate.port_name === draft.usbDevicePort,
			);
			if (!device || !onProvisionUsbDevice) {
				setError("The selected USB DMX device is no longer available. Scan again.");
				return;
			}
			setBusy(true);
			const endpoint = await onProvisionUsbDevice(device, draft.usbDriver);
			setBusy(false);
			if (!endpoint) {
				setError("The USB DMX device could not be prepared for output.");
				return;
			}
			body = {
				...body,
				target: { kind: "usb_endpoint", endpoint_id: endpoint.endpoint_id },
			};
		}
		const destination = usbTarget
			? null
			: draft.body.delivery_mode === "unicast"
				? draft.body.destination?.trim() || null
				: null;
		const routesToSave = logicalUniverses.map((logicalUniverse, index) => ({
			...body,
			logical_universe: logicalUniverse,
			destination_universe: destinationUniverses[index],
			destination,
		}));
		const issue = routesToSave
			.map((route) => validate(route, outputBindIp))
			.find(Boolean);
		if (issue) return setError(issue);
		setBusy(true);
		const saved =
			routesToSave.length === 1
				? await onSave(draft.id, routesToSave[0], draft.revision)
				: await onCreateRange({
						logical_start: logicalUniverses[0],
						logical_end: logicalEnd,
						destination_start: destinationUniverses[0],
						destination_end: destinationEnd,
						route: {
							protocol: routesToSave[0].protocol,
							delivery_mode: routesToSave[0].delivery_mode,
							destination: routesToSave[0].destination,
							enabled: routesToSave[0].enabled,
							minimum_slots: routesToSave[0].minimum_slots,
						},
					});
		setBusy(false);
		if (saved) close();
		else
			setError(
				"The route range was not saved. Check its universes and destination, then refresh after a revision conflict.",
			);
	};
	const remove = async () => {
		if (!draft || draft.revision === 0) return close();
		setBusy(true);
		const removed = await onDelete(draft.id, draft.revision);
		setBusy(false);
		if (removed) close();
		else
			setError(
				"The route was not removed. Refresh after a revision conflict and try again.",
			);
	};
	const selectedUsbDevice = draft?.usbDevicePort
		? usbDevices.find((device) => device.port_name === draft.usbDevicePort)
		: undefined;
	const selectedRecommendedDriver = selectedUsbDevice
		? recommendedUsbDmxDriver(selectedUsbDevice)
		: null;

	return (
		<section className="output-routes-setup" aria-label="Output routes">
			<header>
				<div>
					<h3>Routes</h3>
					<small>
						Map logical show universes to Art-Net or sACN destinations.
					</small>
				</div>
				<div className="setup-section-actions">
					<Button
						disabled={usbBusy || !onScanUsbDevices}
						onClick={() => void onScanUsbDevices?.()}
					>
						{usbBusy ? "Scanning…" : "Scan USB devices"}
					</Button>
					<Button
						onClick={() => {
							setDraft(newRoute());
							setError("");
							setConfirmDelete(false);
						}}
					>
						Add route
					</Button>
				</div>
			</header>
			{usbError && (
				<p className="ui-field-error" role="alert">
					USB DMX discovery failed: {usbError}
				</p>
			)}
			{usbDevices.length > 0 && (
				<div
					className="setup-list usb-dmx-device-list"
					aria-label="Discovered USB DMX devices"
				>
					{usbDevices.map((device) => (
						<article key={device.port_name}>
							<span>
								<b>{device.identity.product ?? "USB DMX device"}</b>
								<small>
									{device.identity.usb_serial
										? `USB serial ${device.identity.usb_serial}`
										: "USB serial unavailable"}
									{device.port_name.startsWith("/dev/cu.")
										? ` · Recommended macOS connection ${device.port_name}`
										: ` · ${device.port_name}`}
								</small>
							</span>
							<Button
								disabled={usbBusy}
								onClick={() => {
									setDraft(newRoute(device));
									setError("");
									setConfirmDelete(false);
								}}
							>
								Add route for device
							</Button>
						</article>
					))}
				</div>
			)}
			<div className="setup-list output-route-list">
				{ordered.map((route) => (
					<article key={route.id}>
						<span>
							<b>
								Logical {route.body.logical_universe} →{" "}
								{route.body.target?.kind === "usb_endpoint"
									? endpointLabel(routeUsbEndpoint(usbEndpoints, route.body))
									: `${route.body.protocol === "art_net" ? "Art-Net" : "sACN"} ${route.body.destination_universe}`}
							</b>
							<small>
								{modeLabel(route.body)} ·{" "}
								{route.body.target?.kind === "usb_endpoint"
									? "Final DMX frame over the connected USB device"
									: route.body.destination ||
										(route.body.protocol === "art_net"
											? "255.255.255.255:6454"
											: `239.255.${route.body.destination_universe >> 8}.${route.body.destination_universe & 255}:5568`)}{" "}
								· Minimum {route.body.minimum_slots ?? 512} slots
							</small>
						</span>
						<span
							className={
								route.body.enabled ? "route-enabled" : "route-disabled"
							}
						>
							{route.body.enabled ? "Enabled" : "Disabled"}
						</span>
						<Button onClick={() => edit(route)}>Edit route</Button>
					</article>
				))}
				{!ordered.length && (
					<p className="empty-window-message">
						No output routes are configured.
					</p>
				)}
			</div>
			{draft && (
				<ModalPortal onClose={close}>
					<div
						className="modal-backdrop"
						onPointerDown={(event) =>
							event.target === event.currentTarget && close()
						}
					>
						<section
							className="modal-card output-route-editor"
							role="dialog"
							aria-modal="true"
							aria-label="Output route editor"
						>
							<Button className="modal-close" disabled={busy} onClick={close}>
								×
							</Button>
							<h2>
								{draft.revision ? "Edit output route" : "Add output route"}
							</h2>
							<FormLayout labelPlacement="side">
								<SelectField
									label="Output transport"
									value={draft.body.target?.kind ?? "network"}
									onChange={(kind) => {
										const device = usbDevices[0];
										setDraft({
											...draft,
											usbDevicePort:
												kind === "usb_endpoint" ? (device?.port_name ?? null) : null,
											usbDriver:
												(device && recommendedUsbDmxDriver(device)) ??
												draft.usbDriver,
											body: {
												...draft.body,
												target:
													kind === "usb_endpoint"
														? {
																kind,
																endpoint_id:
																	device == null
																		? (usbEndpoints[0]?.endpoint_id ?? "")
																		: "",
															}
														: undefined,
											},
										});
									}}
									options={[
										{ value: "network", label: "Network (Art-Net / sACN)" },
										{
											value: "usb_endpoint",
											label: "USB DMX device",
											disabled: !usbEndpoints.length && !usbDevices.length,
										},
									]}
								/>
								{draft.body.target?.kind === "usb_endpoint" && (
									<>
										<SelectField
											label="USB DMX device"
											value={
												draft.usbDevicePort
													? `device:${draft.usbDevicePort}`
													: `configured:${draft.body.target.endpoint_id}`
											}
											onChange={(value) => {
												const discovered = value.startsWith("device:");
												const identity = value.slice(value.indexOf(":") + 1);
												const device = discovered
													? usbDevices.find(
															(candidate) => candidate.port_name === identity,
														)
													: undefined;
												setDraft({
													...draft,
													usbDevicePort: device?.port_name ?? null,
													usbDriver:
														(device && recommendedUsbDmxDriver(device)) ??
														draft.usbDriver,
													body: {
														...draft.body,
														target: {
															kind: "usb_endpoint",
															endpoint_id: discovered ? "" : identity,
														},
													},
												});
											}}
											options={[
												...usbDevices.map((device) => ({
													value: `device:${device.port_name}`,
													label: usbDmxDeviceLabel(device),
												})),
												...usbEndpoints.map((endpoint) => ({
													value: `configured:${endpoint.endpoint_id}`,
													label: endpointLabel(endpoint),
													disabled: !endpoint.enabled,
												})),
											]}
										/>
										{selectedUsbDevice && selectedRecommendedDriver ? (
											<p className="field-description">
												{selectedRecommendedDriver === "enttec_usb_pro_v144"
													? "ENTTEC USB Pro selected from the device metadata."
													: "Open DMX selected from the device metadata."}
											</p>
										) : selectedUsbDevice ? (
							<SelectField
								label="USB DMX device type"
								value={draft.usbDriver}
								onChange={(usbDriver) =>
									setDraft({
										...draft,
										usbDriver: usbDriver as UsbDmxDriverKind,
									})
								}
												options={[
													{
														value: "enttec_usb_pro_v144",
														label: "ENTTEC USB Pro",
													},
													{ value: "open_dmx", label: "Open DMX (FTDI)" },
												]}
											/>
										) : null}
									</>
								)}
								{draft.body.target?.kind !== "usb_endpoint" && (
									<>
										<SelectField
											label="Protocol"
											value={draft.body.protocol}
											onChange={(protocol) =>
												setDraft({
													...draft,
													body: {
														...draft.body,
														protocol,
														delivery_mode:
															protocol === "art_net"
																? "broadcast"
																: "multicast",
														destination: null,
													},
												})
											}
											options={[
												{ value: "art_net", label: "Art-Net" },
												{ value: "sacn", label: "sACN" },
											]}
										/>
										<SelectField
											label="Delivery mode"
											value={draft.body.delivery_mode}
											onChange={(delivery_mode) =>
												setDraft({
													...draft,
													body: {
														...draft.body,
														delivery_mode,
														destination:
															delivery_mode === "unicast"
																? draft.body.destination
																: null,
													},
												})
											}
											options={
												draft.body.protocol === "art_net"
													? [
															{ value: "broadcast", label: "Broadcast" },
															{ value: "unicast", label: "Unicast" },
														]
													: [
															{ value: "multicast", label: "Multicast" },
															{ value: "unicast", label: "Unicast" },
														]
											}
										/>
									</>
								)}
								<NumberField
									label="Logical universe"
									min="1"
									max="65535"
									value={draft.logicalUniverse}
									allowThrough={draft.revision === 0}
									description={
										draft.revision
											? "One universe while editing."
											: "One universe or a paired range, for example 1 THRU 8."
									}
									onChange={(event) =>
										setDraft({ ...draft, logicalUniverse: event.target.value })
									}
									onRangeCommit={(points) =>
										setDraft({
											...draft,
											logicalUniverse: points.join(" THRU "),
										})
									}
								/>
								{draft.body.target?.kind !== "usb_endpoint" && (
									<NumberField
										label="Destination universe"
										min="1"
										max={draft.body.protocol === "art_net" ? "32767" : "63999"}
										value={draft.destinationUniverse}
										allowThrough={draft.revision === 0}
										description={
											draft.revision
												? "One universe while editing."
												: "Use the same range length as the logical universes."
										}
										onChange={(event) =>
											setDraft({
												...draft,
												destinationUniverse: event.target.value,
											})
										}
										onRangeCommit={(points) =>
											setDraft({
												...draft,
												destinationUniverse: points.join(" THRU "),
											})
										}
									/>
								)}
								<NumberField
									label="Minimum universe size"
									min="1"
									max="512"
									value={draft.body.minimum_slots ?? 512}
									description="Enabled routes send at least this many slots. Patched fixtures extend the frame when needed."
									onChange={(event) =>
										setDraft({
											...draft,
											body: {
												...draft.body,
												minimum_slots: Number(event.target.value),
											},
										})
									}
								/>
								{draft.body.target?.kind !== "usb_endpoint" &&
									draft.body.delivery_mode === "unicast" && (
										<TextField
											label="Destination"
											value={draft.body.destination ?? ""}
											description="Required IPv4 address and port, for example 10.0.0.20:6454."
											onChange={(event) =>
												setDraft({
													...draft,
													body: {
														...draft.body,
														destination: event.target.value,
													},
												})
											}
										/>
									)}
								{draft.body.target?.kind !== "usb_endpoint" &&
									draft.body.delivery_mode === "broadcast" && (
										<p className="field-description">
											Art-Net Broadcast uses the global destination
											255.255.255.255:6454. The desk's output bind address
											selects the lighting-network interface.
										</p>
									)}
								{draft.body.target?.kind !== "usb_endpoint" &&
									draft.body.delivery_mode === "multicast" && (
										<p className="field-description">
											sACN Multicast derives its 239.255.x.y:5568 destination
											from the destination universe.
										</p>
									)}
								<SwitchField
									label="Route state"
									offLabel="Disabled"
									onLabel="Enabled"
									checked={draft.body.enabled}
									onChange={(event) =>
										setDraft({
											...draft,
											body: { ...draft.body, enabled: event.target.checked },
										})
									}
								/>
							</FormLayout>
							{error && (
								<p className="ui-field-error" role="alert">
									{error}
								</p>
							)}
							{confirmDelete ? (
								<div className="delete-confirm">
									<b>Remove this output route?</b>
									<Button
										disabled={busy}
										onClick={() => setConfirmDelete(false)}
									>
										Cancel
									</Button>
									<Button
										className="danger"
										disabled={busy}
										onClick={() => void remove()}
									>
										Confirm remove
									</Button>
								</div>
							) : (
								<footer className="modal-actions">
									{draft.revision > 0 && (
										<Button
											className="danger"
											disabled={busy}
											onClick={() => setConfirmDelete(true)}
										>
											Remove route
										</Button>
									)}
									<Button disabled={busy} onClick={close}>
										Cancel
									</Button>
									<Button
										variant="primary"
										disabled={busy}
										onClick={() => void save()}
									>
										{busy ? "Saving…" : "Save route"}
									</Button>
								</footer>
							)}
						</section>
					</div>
				</ModalPortal>
			)}
		</section>
	);
}
