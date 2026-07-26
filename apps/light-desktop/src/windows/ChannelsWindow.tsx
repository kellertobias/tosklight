import { type CSSProperties, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { usePatchedFixturesView } from "../features/patch/PatchState";
import type { PatchedFixture, VisualizationSnapshot } from "../api/types";
import { Button, ModalRegistration } from "@tosklight/ui";
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

const DEFAULT_COLUMNS = 10;
const ROWS = 2;
const DEFAULT_PAGE_SIZE = DEFAULT_COLUMNS * ROWS;

export interface Channel {
	number: number;
	fixture: PatchedFixture;
	name: string;
	level: number;
}

export function ChannelsWindow({ active = true, compact }: WindowProps) {
	const selection = useProgrammingSelectionView(active);
	const selectionActions = useProgrammingSelectionActions(active);
	const values = useProgrammerValuesMutationQueue(active);
	const [page, setPage] = useState(0);
	const [pagePickerOpen, setPagePickerOpen] = useState(false);
	const [columns, setColumns] = useState(DEFAULT_COLUMNS);
	const visualization = useChannelVisualization(active);
	const selectedFixtureIds = useMemo(
		() => new Set(selection?.selected ?? []),
		[selection?.selected],
	);
	const fixtures = usePatchedFixturesView(active);
	const channels = channelProjection(fixtures, visualization);
	const pages = Math.max(8, Math.ceil(channels.length / DEFAULT_PAGE_SIZE));
	const setIntensity = (fixtureId: string, level: number) => {
		const mutations = normalizedFixtureMutations(
			[{ fixtureId, attribute: "intensity", value: level }],
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
			onPage={setPage}
			onPagePickerOpen={setPagePickerOpen}
			columns={columns}
			onColumnsChange={(next) => {
				setColumns(next);
				setPage(0);
			}}
			onSelect={(fixtureId) =>
				void selectionActions?.replace({ resolvedFixtures: [fixtureId] })
			}
			onSetIntensity={(fixtureId, level) => void setIntensity(fixtureId, level)}
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
	onPage,
	onPagePickerOpen,
	onSelect,
	onSetIntensity,
	columns = DEFAULT_COLUMNS,
	onColumnsChange,
}: {
	channels: readonly Channel[];
	compact?: boolean;
	page: number;
	pages: number;
	pagePickerOpen: boolean;
	selectedFixtureIds: ReadonlySet<string>;
	valuesReady: boolean;
	onPage(page: number): void;
	onPagePickerOpen(open: boolean): void;
	onSelect(fixtureId: string): void;
	onSetIntensity(fixtureId: string, level: number): void;
	columns?: number;
	onColumnsChange?(columns: number): void;
}) {
	usePagePickerDismissal(pagePickerOpen, onPagePickerOpen);
	const pageSize = columns * ROWS;
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	return (
		<div className="channels-window">
			{!compact && (
				<ChannelHeader
					columns={columns}
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
				onSelect={onSelect}
				onSetIntensity={onSetIntensity}
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
	});
}

function channelProjection(
	fixtures: readonly PatchedFixture[],
	visualization: VisualizationSnapshot | null,
): Channel[] {
	return fixtures.map((fixture, index) => ({
		number: index + 1,
		fixture,
		name: fixture.definition.name ?? fixture.definition.model,
		level: Math.round(fixtureValue(visualization, fixture, "intensity") * 100),
	}));
}

function ChannelHeader({
	columns,
	page,
	pages,
	onPage,
	onOpenPicker,
	onSettings,
}: {
	columns: number;
	page: number;
	pages: number;
	onPage(page: number): void;
	onOpenPicker(): void;
	onSettings?(anchor: DOMRect): void;
}) {
	return (
		<WindowHeader
			title="Channels"
			info={{ primary: "Intensity", secondary: "Two-row channel bank" }}
			actions={[
				[
					{
						id: "previous",
						label: "←",
						disabled: page === 0,
						ariaLabel: "Previous channel page",
						onClick: () => onPage(page - 1),
					},
					{
						id: "page",
						label: pageLabel(page, columns),
						onClick: onOpenPicker,
					},
					{
						id: "next",
						label: "→",
						disabled: page >= pages - 1,
						ariaLabel: "Next channel page",
						onClick: () => onPage(page + 1),
					},
				],
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
	onSelect,
	onSetIntensity,
}: {
	channels: readonly Channel[];
	columns: number;
	page: number;
	selectedFixtureIds: ReadonlySet<string>;
	valuesReady: boolean;
	onSelect(fixtureId: string): void;
	onSetIntensity(fixtureId: string, level: number): void;
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
				return (
					<article
						className={`channel-fader ${channel ? "" : "empty"} ${channel && selectedFixtureIds.has(channel.fixture.fixture_id) ? "selected" : ""}`}
						key={channel?.fixture.fixture_id ?? `empty-${number}`}
						onClick={() => channel && onSelect(channel.fixture.fixture_id)}
					>
						<VerticalTouchFader
							disabled={!channel || !valuesReady}
							label={channel ? `CH ${number}` : `CH ${number} · Empty`}
							mode={channel?.name ?? "Unpatched"}
							value={channel?.level ?? 0}
							display={channel ? `${channel.level}%` : "—"}
							onChange={(value) =>
								channel &&
								onSetIntensity(channel.fixture.fixture_id, value / 100)
							}
						/>
					</article>
				);
			})}
		</FaderView>
	);
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
					<Button className="modal-close" onClick={onClose}>
						×
					</Button>
					<h3>Channel pages</h3>
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
