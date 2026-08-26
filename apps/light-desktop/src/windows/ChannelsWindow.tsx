import { type CSSProperties, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import {
	type PatchStatus,
	usePatchedFixturesView,
	usePatchStatus,
} from "../features/patch/PatchState";
import type { PatchedFixture, VisualizationSnapshot } from "../api/types";
import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { VerticalTouchFader } from "../components/control/VerticalTouchFader";
import {
	FaderView,
	WindowHeader,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../features/programmingInteraction/ProgrammingInteractionView";
import {
	normalizedFixtureMutations,
	programmerValuesMutationKey,
	useProgrammerValuesMutationQueue,
} from "../features/programmerValues/useProgrammerValuesMutationQueue";
import { useVisualizationRuntimeSnapshot } from "../features/visualizationRuntime/VisualizationRuntimeView";
import { fixtureValue } from "./fixtureVisualization";
import type { WindowProps } from "./windowTypes";
import {
	compareFixtureIds,
	fixtureDisplayId,
} from "../components/setup/fixturePatch/fixtureIds";
import { useAttributeRegistry } from "../features/deskSnapshot/DeskSnapshotState";
import type { ChannelDisplayMode } from "../types";

const DEFAULT_COLUMNS = 10;
const ROWS = 2;

export interface Channel {
	number: number;
	fixture: PatchedFixture;
	/** The fixture's operator-facing name, which is what identifies it on the bank. */
	fixtureLabel: string;
	/** The addressable Fixture ID, kept beside the name for command-line addressing. */
	fixtureId: string;
	attribute: string;
	attributeLabel: string;
	level: number;
}

/**
 * The name an operator gave the fixture, falling back to the profile name.
 *
 * An unnamed fixture still has to be identifiable, so the Fixture ID is the last resort rather
 * than the default label.
 */
export function channelFixtureLabel(fixture: PatchedFixture): string {
	return (
		fixture.name?.trim() ||
		fixture.definition.name?.trim() ||
		`Fixture ${fixtureDisplayId(fixture)}`
	);
}

export function ChannelsWindow({
	active = true,
	compact,
	channelDisplayMode,
}: WindowProps) {
	const selection = useProgrammingSelectionView(active);
	const selectionActions = useProgrammingSelectionActions(active);
	const values = useProgrammerValuesMutationQueue(active);
	const [page, setPage] = useState(0);
	const [pagePickerOpen, setPagePickerOpen] = useState(false);
	const [columns, setColumns] = useState(DEFAULT_COLUMNS);
	const [standaloneDisplayMode, setStandaloneDisplayMode] =
		useState<ChannelDisplayMode>("intensity");
	const displayMode = channelDisplayMode ?? standaloneDisplayMode;
	const visualization = useChannelVisualization(active);
	const attributeRegistry = useAttributeRegistry();
	const selectedFixtureIds = useMemo(
		() => new Set(selection?.selected ?? []),
		[selection?.selected],
	);
	const fixtures = usePatchedFixturesView(active);
	const patchStatus = usePatchStatus(active);
	const channels = channelProjection(
		fixtures,
		visualization,
		displayMode,
		attributeRegistry ?? [],
	);
	const pages = Math.max(8, Math.ceil(channels.length / (columns * ROWS)));
	const setChannelValue = (
		fixtureId: string,
		attribute: string,
		level: number,
	) => {
		const mutations = normalizedFixtureMutations(
			[{ fixtureId, attribute, value: level }],
			undefined,
			true,
		);
		return values.submitLatest(
			programmerValuesMutationKey(mutations),
			mutations,
		);
	};
	return (
		<ChannelsWindowView
			channels={channels}
			compact={compact}
			page={page}
			pages={pages}
			pagePickerOpen={pagePickerOpen}
			selectedFixtureIds={selectedFixtureIds}
			valuesReady={values.canWrite}
			valuesUnavailableReason={values.unavailableReason}
			patchUnavailableReason={channelPatchUnavailableReason(active, patchStatus)}
			onPage={setPage}
			onPagePickerOpen={setPagePickerOpen}
			columns={columns}
			displayMode={displayMode}
			onDisplayModeChange={
				channelDisplayMode == null
					? (mode) => {
							setStandaloneDisplayMode(mode);
							setPage(0);
						}
					: undefined
			}
			onColumnsChange={(next) => {
				setColumns(next);
				setPage(0);
			}}
			onSelect={(fixtureId) =>
				void selectionActions?.replace({ resolvedFixtures: [fixtureId] })
			}
			onSetValue={(fixtureId, attribute, level) =>
				void setChannelValue(fixtureId, attribute, level)
			}
		/>
	);
}

export function ChannelsWindowView({
	channels,
	compact,
	page,
	pages,
	pagePickerOpen,
	selectedFixtureIds,
	valuesReady,
	valuesUnavailableReason,
	patchUnavailableReason,
	onPage,
	onPagePickerOpen,
	onSelect,
	onSetValue,
	columns = DEFAULT_COLUMNS,
	onColumnsChange,
	displayMode = "intensity",
	onDisplayModeChange,
}: {
	channels: readonly Channel[];
	compact?: boolean;
	page: number;
	pages: number;
	pagePickerOpen: boolean;
	selectedFixtureIds: ReadonlySet<string>;
	valuesReady: boolean;
	valuesUnavailableReason?: string | null;
	patchUnavailableReason?: string | null;
	onPage(page: number): void;
	onPagePickerOpen(open: boolean): void;
	onSelect(fixtureId: string): void;
	onSetValue(fixtureId: string, attribute: string, level: number): void;
	columns?: number;
	onColumnsChange?(columns: number): void;
	displayMode?: ChannelDisplayMode;
	onDisplayModeChange?(mode: ChannelDisplayMode): void;
}) {
	usePagePickerDismissal(pagePickerOpen, onPagePickerOpen);
	const pageSize = columns * ROWS;
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	return (
		<div className="channels-window">
			{!compact && (
				<ChannelHeader
					columns={columns}
					displayMode={displayMode}
					page={page}
					pages={pages}
					onPage={onPage}
					onOpenPicker={() => onPagePickerOpen(true)}
					onSettings={
						onColumnsChange ? (anchor) => setSettingsAnchor(anchor) : undefined
					}
				/>
			)}
			{settingsAnchor && onColumnsChange && (
				<WindowSettings
					modal={false}
					anchor={settingsAnchor}
					title="Channel Settings"
					onClose={() => setSettingsAnchor(null)}
					tabs={[
						{
							id: "layout",
							label: "Layout",
							content: (
								<>
									{onDisplayModeChange && (
										<>
											<h3>Displayed channels</h3>
											<div className="button-group">
												<Button
													className={
														displayMode === "intensity" ? "active" : ""
													}
													onClick={() => onDisplayModeChange("intensity")}
												>
													Intensity only
												</Button>
												<Button
													className={displayMode === "all" ? "active" : ""}
													onClick={() => onDisplayModeChange("all")}
												>
													All channels
												</Button>
											</div>
										</>
									)}
									<h3>Channels per row</h3>
									<div className="button-group">
										{[6, 8, 10].map((count) => (
											<Button
												className={columns === count ? "active" : ""}
												key={count}
												onClick={() => onColumnsChange(count)}
											>
												{count}
											</Button>
										))}
									</div>
								</>
							),
						},
					]}
				/>
			)}
			<ChannelFaderBank
				channels={channels.slice(page * pageSize, (page + 1) * pageSize)}
				columns={columns}
				page={page}
				selectedFixtureIds={selectedFixtureIds}
				valuesReady={valuesReady}
				valuesUnavailableReason={valuesUnavailableReason}
				patchUnavailableReason={patchUnavailableReason}
				onSelect={onSelect}
				onSetValue={onSetValue}
			/>
			{pagePickerOpen && (
				<ChannelPagePicker
					columns={columns}
					page={page}
					pages={pages}
					onPage={onPage}
					onClose={() => onPagePickerOpen(false)}
				/>
			)}
		</div>
	);
}

function useChannelVisualization(active: boolean) {
	return useVisualizationRuntimeSnapshot({
		enabled: active,
		intervalMillis: 250,
		consumerId: "channels",
	});
}

export function channelProjection(
	fixtures: readonly PatchedFixture[],
	visualization: VisualizationSnapshot | null,
	displayMode: ChannelDisplayMode = "intensity",
	attributeRegistry: readonly { id: string; label: string }[] = [],
): Channel[] {
	const labels = new Map(
		attributeRegistry.map((descriptor) => [descriptor.id, descriptor.label]),
	);
	return [...fixtures]
		.sort(compareFixtureIds)
		.flatMap((fixture) => {
			const allAttributes = fixture.definition.heads.flatMap((head) =>
				head.parameters.map((parameter) => parameter.attribute),
			);
			const attributes =
				displayMode === "intensity"
					? ["intensity"]
					: [...new Set(allAttributes.length ? allAttributes : ["intensity"])];
			return attributes.map((attribute) => ({
				number: 0,
				fixture,
				fixtureLabel: channelFixtureLabel(fixture),
				fixtureId: String(fixtureDisplayId(fixture)),
				attribute,
				attributeLabel: labels.get(attribute) ?? attributeFallbackLabel(attribute),
				level: Math.round(
					fixtureValue(visualization, fixture, attribute) * 100,
				),
			}));
		})
		.map((channel, index) => ({ ...channel, number: index + 1 }));
}

function attributeFallbackLabel(attribute: string) {
	return attribute
		.split(".")
		.at(-1)!
		.replaceAll("_", " ")
		.replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function ChannelHeader({
	columns,
	displayMode,
	page,
	pages,
	onPage,
	onOpenPicker,
	onSettings,
}: {
	columns: number;
	displayMode: ChannelDisplayMode;
	page: number;
	pages: number;
	onPage(page: number): void;
	onOpenPicker(): void;
	onSettings?(anchor: DOMRect): void;
}) {
	return (
		<WindowHeader
			title="Channels"
			info={{
				primary: displayMode === "intensity" ? "Intensity only" : "All channels",
				secondary: "Fixture ID order · left to right, then down",
			}}
			groups={[
				{ id: "channel-page", actions: [
					{
						id: "previous",
						label: "←",
						disabled: page === 0,
						ariaLabel: "Previous channel page",
						onPress: () => onPage(page - 1),
					},
					{
						id: "page",
						label: pageLabel(page, columns),
						onPress: onOpenPicker,
					},
					{
						id: "next",
						label: "→",
						disabled: page >= pages - 1,
						ariaLabel: "Next channel page",
						onPress: () => onPage(page + 1),
					},
				] },
			]}
			settings={Boolean(onSettings)}
			onSettings={(button) => onSettings?.(button.getBoundingClientRect())}
		/>
	);
}

function ChannelFaderBank({
	channels,
	columns,
	page,
	selectedFixtureIds,
	valuesReady,
	valuesUnavailableReason,
	patchUnavailableReason,
	onSelect,
	onSetValue,
}: {
	channels: readonly Channel[];
	columns: number;
	page: number;
	selectedFixtureIds: ReadonlySet<string>;
	valuesReady: boolean;
	valuesUnavailableReason?: string | null;
	patchUnavailableReason?: string | null;
	onSelect(fixtureId: string): void;
	onSetValue(fixtureId: string, attribute: string, level: number): void;
}) {
	const visible = Array.from(
		{ length: columns * ROWS },
		(_, index) => channels[index] ?? null,
	);
	return (
		<FaderView
			rows={ROWS}
			className="channel-fader-bank"
			style={{ "--channel-columns": columns } as CSSProperties}
		>
			{visible.map((channel, index) => {
				const number = page * columns * ROWS + index + 1;
				const disabledReason = channelFaderDisabledReason(
					Boolean(channel),
					valuesReady,
					valuesUnavailableReason,
					patchUnavailableReason,
				);
				return (
					<article
						className={`channel-fader ${channel ? "" : "empty"} ${channel && selectedFixtureIds.has(channel.fixture.fixture_id) ? "selected" : ""}`}
						key={
							channel
								? `${channel.fixture.fixture_id}:${channel.attribute}`
								: `empty-${number}`
						}
						onClick={() => channel && onSelect(channel.fixture.fixture_id)}
						aria-label={
							channel
								? `Channel ${number}: Fixture ${channel.fixtureId} ${channel.fixtureLabel} ${channel.attributeLabel}`
								: `Channel ${number}: empty`
						}
					>
						{channel && <small className="channel-fader-id">{channel.fixtureId}</small>}
						<VerticalTouchFader
							disabled={disabledReason !== null}
							label={channel ? channel.fixtureLabel : "Empty"}
							mode={disabledReason ?? channel?.attributeLabel ?? "Unpatched"}
							value={channel?.level ?? 0}
							display={channel ? `${channel.level}%` : "—"}
							onChange={(value) =>
								channel &&
								onSetValue(
									channel.fixture.fixture_id,
									channel.attribute,
									value / 100,
								)
							}
						/>
					</article>
				);
			})}
		</FaderView>
	);
}

export function channelPatchUnavailableReason(
	active: boolean,
	patch: PatchStatus,
): string | null {
	if (!active) return "Channel controls are inactive";
	if (patch.status === "loading") return "Patch is loading";
	if (patch.status === "repairing") return "Patch is resynchronizing";
	if (patch.status === "error") return "Patch is unavailable";
	return null;
}

export function channelFaderDisabledReason(
	channelPresent: boolean,
	valuesReady: boolean,
	valuesUnavailableReason?: string | null,
	patchUnavailableReason?: string | null,
): string | null {
	if (!channelPresent) return patchUnavailableReason ?? "Empty position";
	if (!valuesReady)
		return valuesUnavailableReason ?? "Programmer control is unavailable";
	return null;
}

function ChannelPagePicker({
	columns,
	page,
	pages,
	onPage,
	onClose,
}: {
	columns: number;
	page: number;
	pages: number;
	onPage(page: number): void;
	onClose(): void;
}) {
	return createPortal(
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<div
					className="nested-modal channel-page-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Channel pages"
				>
					<ModalTitleBar title="Channel pages" onClose={onClose} />
					<div>
						{Array.from({ length: pages }, (_, nextPage) => (
							<Button
								className={nextPage === page ? "active" : ""}
								key={nextPage}
								onClick={() => {
									onPage(nextPage);
									onClose();
								}}
							>
								{pageLabel(nextPage, columns)}
							</Button>
						))}
					</div>
				</div>
			</div>
		</ModalRegistration>,
		document.body,
	);
}

function usePagePickerDismissal(
	open: boolean,
	setOpen: (open: boolean) => void,
) {
	useEffect(() => {
		if (!open) return;
		const close = (event: KeyboardEvent) => {
			if (event.key !== "Escape") return;
			event.preventDefault();
			setOpen(false);
		};
		window.addEventListener("keydown", close, true);
		return () => window.removeEventListener("keydown", close, true);
	}, [open, setOpen]);
}

function pageLabel(page: number, columns: number) {
	const pageSize = columns * ROWS;
	return `${page * pageSize + 1}–${(page + 1) * pageSize}`;
}
