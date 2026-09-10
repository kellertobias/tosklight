// What the desk is doing to this server.
//
// This page answers one question an operator asks at a patch bay: is anything actually arriving,
// and is it landing where I think it is. It reports; it does not control.

import { WindowFrame } from "@tosklight/ui/window-kit";
import { useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { addressLabel } from "../../entities/catalog";
import { percent, sourceBadge } from "../../entities/output";
import { ApiFailure, api } from "../../shared/api/client";
import type {
	DmxChannelView,
	DmxIngressView,
	DmxMapView,
	OutputView,
} from "../../shared/api/generated/media-wire";
import { useOutputs } from "../../shared/api/queries";
import { useTelemetry } from "../../shared/api/telemetry";

const OUTPUT_POLL_MS = 1_000;

export function DmxPage() {
	const outputs = useOutputs(OUTPUT_POLL_MS);
	const telemetry = useTelemetry();

	return (
		<WindowFrame
			title="Diagnostics"
			className="media-dmx-window"
			info={{ primary: "DMX", secondary: "Input and channel diagnostics" }}
			groups={[
				{
					id: "dmx-diagnostics-actions",
					actions: [
						{
							id: "configure-dmx-input",
							label: "Configure DMX input",
							onPress: () => {
								window.history.pushState(null, "", "/settings?section=dmx");
								window.dispatchEvent(new PopStateEvent("popstate"));
							},
						},
					],
				},
			]}
		>
			<section className="media-page media-dmx-content">
				<FixtureDownloads />
				<ResourceState
					resource={outputs}
					subject="outputs"
					isEmpty={(data) => data.length === 0}
					empty="This server has no enabled outputs, so nothing is listening for a desk."
				>
					{(data) =>
						data.map((output) => (
							<OutputDmx
								key={output.id}
								output={output}
								ingress={telemetry.frame?.dmx.find(
									(sample) => sample.outputId === output.id,
								)}
							/>
						))
					}
				</ResourceState>
			</section>
		</WindowFrame>
	);
}

function FixtureDownloads() {
	const [fixtures, setFixtures] = useState<string[]>([]);
	useEffect(() => {
		let current = true;
		void api
			.fixtures()
			.then((names) => {
				if (current) setFixtures(names);
			})
			.catch(() => undefined);
		return () => {
			current = false;
		};
	}, []);
	if (fixtures.length === 0) return null;
	const gdtf = fixtures.filter((name) => name.endsWith(".gdtf"));
	const magicq = fixtures.filter(
		(name) => name.endsWith(".hed") || name.endsWith(".csv"),
	);
	const grandma2 = fixtures.filter((name) => name.endsWith(".xml"));
	return (
		<section
			className="media-settings-section"
			aria-label="Console personalities"
		>
			<h2>Console personalities</h2>
			<p>
				Patch one Layer personality per Pixel layer, followed immediately by one
				Master personality. Every download uses Pixel’s canonical DMX layout.
			</p>
			<PersonalityGroup
				title="MagicQ"
				note="Install the native .hed files. The channel and range CSVs are developer sources for the exact encoder mapping and indexed Media Folder/File behavior."
				files={magicq}
			/>
			<PersonalityGroup
				title="grandMA2"
				note="Copy the XML files into the gma2/library folder on the selected drive."
				files={grandma2}
			/>
			<PersonalityGroup
				title="grandMA3"
				note="grandMA3 imports the GDTF files directly."
				files={gdtf}
			/>
			<PersonalityGroup
				title="GDTF"
				note="Use these standards-based files with any GDTF-compatible console."
				files={gdtf}
			/>
		</section>
	);
}

function PersonalityGroup({
	title,
	note,
	files,
}: {
	title: string;
	note: string;
	files: string[];
}) {
	if (files.length === 0) return null;
	return (
		// biome-ignore lint/a11y/useSemanticElements: this is a download group, not a form fieldset.
		<div role="group" aria-label={`${title} personalities`}>
			<h3>{title}</h3>
			<p>{note}</p>
			<div className="media-settings-actions">
				{files.map((name) => (
					<a
						className="ui-button ui-secondary ui-default"
						href={api.fixtureUrl(name)}
						download
						key={`${title}-${name}`}
					>
						Download {name}
					</a>
				))}
			</div>
		</div>
	);
}

function OutputDmx({
	output,
	ingress,
}: {
	output: OutputView;
	ingress?: DmxIngressView;
}) {
	const map = useDmxMap(output.id);
	const active = ingress?.active ?? output.dmxActive;
	return (
		<section className="media-output" aria-label={`${output.name} DMX`}>
			<h2>{output.name}</h2>
			<p className={`media-badge is-${active ? "good" : "neutral"}`}>
				{active
					? "A desk is sending to this output"
					: "No desk has sent to this output recently"}
			</p>
			{ingress && <IngressFacts ingress={ingress} />}

			<table className="media-table">
				<caption className="media-visually-hidden">
					Layer values arriving on {output.name}
				</caption>
				<thead>
					<tr>
						<th scope="col">Layer</th>
						<th scope="col">Address</th>
						<th scope="col">Play mode</th>
						<th scope="col">Dimmer</th>
						<th scope="col">Source</th>
					</tr>
				</thead>
				<tbody>
					{output.layers.map((layer) => {
						const badge = sourceBadge(layer.sourceStatus);
						return (
							<tr key={layer.index}>
								<td>{layer.index + 1}</td>
								<td>
									{addressLabel(layer.address.folder, layer.address.file)}
								</td>
								<td>{layer.playMode}</td>
								<td>{percent(layer.dimmer)}</td>
								<td>{badge.label}</td>
							</tr>
						);
					})}
				</tbody>
			</table>

			<dl className="media-facts">
				<dt>Master dimmer</dt>
				<dd>{percent(output.master.dimmer)}</dd>
				<dt>Master volume</dt>
				<dd>{percent(output.master.volume)}</dd>
				<dt>Master tint</dt>
				<dd>
					R {percent(output.master.tintRed)} · G{" "}
					{percent(output.master.tintGreen)} · B{" "}
					{percent(output.master.tintBlue)}
				</dd>
				<dt>Flip and mirror</dt>
				<dd>{output.master.flipMirror}</dd>
				<dt>Master mask</dt>
				<dd>
					{output.master.mask.class === "blank"
						? "none"
						: addressLabel(output.master.mask.folder, output.master.mask.file)}
				</dd>
			</dl>

			{map.failure && (
				<p className="media-state is-error">{map.failure.message}</p>
			)}
			{map.data && <ChannelMap map={map.data} ingress={ingress} />}
		</section>
	);
}

function useDmxMap(output: string): {
	data?: DmxMapView;
	failure?: ApiFailure;
} {
	const [data, setData] = useState<DmxMapView>();
	const [failure, setFailure] = useState<ApiFailure>();
	useEffect(() => {
		let current = true;
		void api
			.dmxMap(output)
			.then((next) => current && setData(next))
			.catch((error: unknown) => {
				if (current)
					setFailure(
						error instanceof ApiFailure
							? error
							: new ApiFailure("unexpected-error", String(error), 0),
					);
			});
		return () => {
			current = false;
		};
	}, [output]);
	return { data, failure };
}

function IngressFacts({ ingress }: { ingress: DmxIngressView }) {
	return (
		<dl className="media-facts">
			<dt>Protocol</dt>
			<dd>{ingress.protocol === "art-net" ? "Art-Net" : "sACN"}</dd>
			<dt>Universe and start</dt>
			<dd>
				{ingress.universe}, address {ingress.startAddress}
			</dd>
			<dt>Winning source</dt>
			<dd>{ingress.source}</dd>
			<dt>Receive rate</dt>
			<dd>{ingress.framesPerSecond.toFixed(1)} fps</dd>
			<dt>Last received</dt>
			<dd>
				{ingress.ageMillis} ms ago{ingress.active ? "" : " — stale"}
			</dd>
		</dl>
	);
}

function ChannelMap({
	map,
	ingress,
}: {
	map: DmxMapView;
	ingress?: DmxIngressView;
}) {
	return (
		<table className="media-table">
			<caption>
				DMX channel map and latest raw values for {map.outputName}
			</caption>
			<thead>
				<tr>
					<th scope="col">Group</th>
					<th scope="col">Channel</th>
					<th scope="col">Name</th>
					<th scope="col">Raw</th>
					<th scope="col">Decoded</th>
				</tr>
			</thead>
			<tbody>
				{map.channels.map((channel) => {
					const value = rawValueOf(channel, map, ingress);
					return (
						<tr key={channel.absoluteChannel}>
							<td>
								{channel.group.kind === "layer"
									? `Layer ${channel.group.number}`
									: "Master"}
							</td>
							<td>{channel.absoluteChannel}</td>
							<td>{channel.name}</td>
							<td className="media-dmx-raw">{value ?? "—"}</td>
							<td>{describeValue(channel, value)}</td>
						</tr>
					);
				})}
			</tbody>
		</table>
	);
}

function rawValueOf(
	channel: DmxChannelView,
	map: DmxMapView,
	ingress?: DmxIngressView,
): number | undefined {
	if (!ingress) return undefined;
	const at = channel.absoluteChannel - map.startAddress;
	const coarse = ingress.slots[at];
	if (coarse === undefined) return undefined;
	if (channel.resolution === "coarse")
		return coarse * 256 + (ingress.slots[at + 1] ?? 0);
	return coarse;
}

function describeValue(channel: DmxChannelView, value?: number): string {
	if (!channel.implemented)
		return channel.implementationNote ?? "Not implemented";
	if (value === undefined) return "No frame received";
	const set = channel.valueSets.find(
		(candidate) =>
			value >= candidate.from &&
			value <= candidate.to &&
			(value - candidate.from) % candidate.step === 0,
	);
	return set?.name ?? String(value);
}
