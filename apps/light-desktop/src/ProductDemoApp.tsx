import {
	type CSSProperties,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import type { DmxSnapshot } from "./api/types";
import { NumericPad } from "./components/control/NumericPad";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { AppShell } from "./components/shell/AppShell";
import { useDmxDiagnostics } from "./features/dmxDiagnostics/DmxDiagnosticsContext";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";
import { DemoPlaybackControls } from "./features/productDemo/DemoPlaybackControls";
import { useConnectionStatus } from "./features/shellStatus/ShellStatusState";
import { AppProvider, useApp } from "./state/AppContext";
import { DEFAULT_STAGE_CAMERA_3D } from "./windows/Stage3dCanvas";
import { StageWindow } from "./windows/StageWindow";

const DEMO_DMX_CHANNELS = 512;
const DEMO_DMX_UNIVERSES = [1, 2, 3, 4] as const;
const DEMO_APPLICATION_WIDTH = 1920;
const DEMO_APPLICATION_ICON = new URL(
	"../src-tauri/icons/icon.png",
	import.meta.url,
).href;
const DEMO_CHAPTERS = [
	["SHOW SETUP", "Show Setup"],
	["OUTPUT CONFIGURATION", "Outputs"],
	["SETTING UP THE BASICS", "Basics"],
	["DEFINING GROUPS", "Groups"],
	["ASSIGNING GROUP MASTERS", "Masters"],
	["PRESET SETUP", "Presets"],
	["Programming Cues & Cuelists", "Cues & Cuelists"],
	["DYNAMICS", "Dynamics"],
	["VIRTUAL PLAYBACKS", "Virtual Playbacks"],
	["Busking", "Busking"],
] as const;

function DemoDmxGrid({ universeNumber }: { universeNumber: number }) {
	const dmx = useDmxDiagnostics();
	const connectionStatus = useConnectionStatus();
	const [snapshot, setSnapshot] = useState<DmxSnapshot | null>(null);
	useEffect(() => {
		if (connectionStatus !== "connected") return;
		let cancelled = false;
		const refresh = () =>
			void dmx
				?.readDmx()
				.then((next) => {
					if (!cancelled) setSnapshot(next);
				})
				.catch(() => undefined);
		refresh();
		const timer = window.setInterval(refresh, 150);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [connectionStatus, dmx]);
	const universe = snapshot?.universes.find(
		(frame) => frame.universe === universeNumber,
	);
	const slots = useMemo(
		() =>
			Array.from(
				{ length: DEMO_DMX_CHANNELS },
				(_, index) => universe?.slots[index] ?? 0,
			),
		[universe],
	);
	return <DemoDmxGridView universeNumber={universeNumber} slots={slots} />;
}

export function DemoDmxGridView({
	universeNumber,
	slots,
}: {
	universeNumber: number;
	slots: readonly number[];
}) {
	return (
		<section
			aria-label={`Live DMX universe ${universeNumber}`}
			className="product-demo-dmx-universe"
		>
			<div className="product-demo-dmx-universe-label">
				UNIVERSE {universeNumber}
			</div>
			<div className="product-demo-dmx-grid">
				{slots.map((value, index) => (
					<span
						aria-label={`DMX ${universeNumber}.${index + 1}: ${value}`}
						className="product-demo-dmx-cell"
						data-address={index + 1}
						data-value={value}
						key={index}
						role="img"
						style={
							{
								"--demo-dmx-level": Math.max(0.07, value / 255),
							} as CSSProperties
						}
					/>
				))}
			</div>
		</section>
	);
}

function DemoCard({
	className,
	title,
	meta,
	children,
}: {
	className: string;
	title: string;
	meta: string;
	children: React.ReactNode;
}) {
	return (
		<section className={`product-demo-card ${className}`}>
			<header>
				<b>{title}</b>
				<span>{meta}</span>
			</header>
			<div className="product-demo-card-body">{children}</div>
		</section>
	);
}

function DemoNarrative() {
	const [playtimeSeconds, setPlaytimeSeconds] = useState(0);
	useEffect(() => {
		const startedAt = performance.now();
		const update = () =>
			setPlaytimeSeconds((performance.now() - startedAt) / 1_000);
		update();
		const timer = window.setInterval(update, 200);
		return () => window.clearInterval(timer);
	}, []);
	return (
		<section
			className="product-demo-narrative"
			aria-label="Product demo progress"
		>
			<img
				src={DEMO_APPLICATION_ICON}
				alt=""
				data-demo-application-icon
				hidden
			/>
			<ol className="product-demo-chapters" data-demo-chapter-strip>
				{DEMO_CHAPTERS.map(([chapter, label], index) => (
					<li key={chapter}>
						<span className="product-demo-chapter" data-demo-chapter={chapter}>
							{label}
						</span>
						{index < DEMO_CHAPTERS.length - 1 && (
							<span className="product-demo-chapter-arrow" aria-hidden="true">
								→
							</span>
						)}
					</li>
				))}
			</ol>
			<div className="product-demo-narrative-line" />
			<div className="product-demo-section-status">
				<div>
					<small>SECTION</small>
					<strong data-demo-section-title>STARTING</strong>
					<span data-demo-subsection />
				</div>
				<time data-demo-playtime dateTime={`PT${playtimeSeconds.toFixed(1)}S`}>
					{formatDemoPlaytime(playtimeSeconds)}
				</time>
			</div>
			<div className="product-demo-current-action">
				<small>CURRENT ACTION</small>
				<strong data-demo-current-action>Preparing the product demo.</strong>
			</div>
		</section>
	);
}

function formatDemoPlaytime(seconds: number) {
	const wholeSeconds = Math.max(0, Math.floor(seconds));
	const minutes = Math.floor(wholeSeconds / 60);
	return `${String(minutes).padStart(2, "0")}:${String(wholeSeconds % 60).padStart(2, "0")}`;
}

function DemoApplicationScreen() {
	return (
		<DemoApplicationScreenView>
			<AppShell />
		</DemoApplicationScreenView>
	);
}

export function DemoApplicationScreenView({
	children,
}: {
	children: React.ReactNode;
}) {
	const viewport = useRef<HTMLElement>(null);
	const [scale, setScale] = useState(1);
	useLayoutEffect(() => {
		const element = viewport.current;
		if (!element) return;
		const resize = () => setScale(element.clientWidth / DEMO_APPLICATION_WIDTH);
		const publishBounds = () => {
			const bounds = element.getBoundingClientRect();
			document.documentElement.style.setProperty(
				"--product-demo-app-scale",
				String(bounds.width / DEMO_APPLICATION_WIDTH),
			);
			document.documentElement.style.setProperty(
				"--product-demo-app-left",
				`${bounds.left}px`,
			);
			document.documentElement.style.setProperty(
				"--product-demo-app-top",
				`${bounds.top}px`,
			);
			document.documentElement.style.setProperty(
				"--product-demo-app-width",
				`${bounds.width}px`,
			);
			document.documentElement.style.setProperty(
				"--product-demo-app-height",
				`${bounds.height}px`,
			);
		};
		const sync = () => {
			resize();
			publishBounds();
		};
		const observer = new ResizeObserver(sync);
		observer.observe(element);
		window.addEventListener("resize", sync);
		sync();
		return () => {
			observer.disconnect();
			window.removeEventListener("resize", sync);
		};
	}, []);
	return (
		<section
			className="product-demo-application"
			aria-label="ToskLight application"
			ref={viewport}
		>
			<div
				className="product-demo-application-canvas"
				style={{ "--demo-application-scale": scale } as CSSProperties}
			>
				{children}
			</div>
		</section>
	);
}

function ProductDemoSurface() {
	const { state, dispatch } = useApp();
	useEffect(() => {
		if (!state.midiProfile) dispatch({ type: "SET_MIDI_PROFILE", value: true });
	}, [state.midiProfile, dispatch]);
	return (
		<ProductDemoSurfaceView
			application={<DemoApplicationScreen />}
			stage={
				<StageWindow
					compact
					stageView="3d"
					showGroupShortcuts={false}
					followPreload={false}
					stageRenderQuality="lines_and_beams"
					showSelection
					showFloorGrid={false}
					showBeamGuides={state.builtIn === "patch"}
					environmentBrightness={1}
					camera3d={DEFAULT_STAGE_CAMERA_3D}
				/>
			}
			dmx={DEMO_DMX_UNIVERSES.map((universeNumber) => (
				<DemoDmxGrid universeNumber={universeNumber} key={universeNumber} />
			))}
			playbackControls={<DemoPlaybackControls />}
			programmer={<NumericPad demo />}
		/>
	);
}

export interface ProductDemoSurfaceViewProps {
	application: React.ReactNode;
	stage: React.ReactNode;
	dmx: React.ReactNode;
	playbackControls: React.ReactNode;
	programmer: React.ReactNode;
}

/** Production marketing composition with runtime-owned surfaces supplied at its data boundary. */
export function ProductDemoSurfaceView({
	application,
	stage,
	dmx,
	playbackControls,
	programmer,
}: ProductDemoSurfaceViewProps) {
	return (
		<main className="product-demo-shell" data-testid="product-demo">
			<section className="product-demo-primary">
				<div className="product-demo-screen-frame">{application}</div>
				<DemoNarrative />
			</section>
			<aside className="product-demo-companion" aria-label="Virtual demo desk">
				<DemoCard
					className="product-demo-stage"
					title="STAGE · 3D"
					meta="FOLLOW SELECTION · GROUPS OFF · ENV 100%"
				>
					{stage}
				</DemoCard>
				<div className="product-demo-visual-divider">
					<span>⌃&nbsp; STAGE RENDER</span>
					<span>LIVE DMX OUTPUT &nbsp;⌄</span>
				</div>
				<DemoCard
					className="product-demo-dmx"
					title="DMX OUTPUT"
					meta="UNIVERSES 1–4 · LIVE"
				>
					{dmx}
				</DemoCard>
				<div className="product-demo-visual-divider">
					<span>⌃&nbsp; LIVE DMX OUTPUT</span>
					<span>SIMULATED HARDWARE CONTROLS &nbsp;⌄</span>
				</div>
				<section className="product-demo-controls">
					{playbackControls}
					<DemoCard
						className="product-demo-programmer"
						title="VIRTUAL DESK"
						meta="PROGRAMMER"
					>
						{programmer}
					</DemoCard>
				</section>
			</aside>
		</main>
	);
}

export function ProductDemoApp() {
	return (
		<ServerRuntime>
			<AppProvider>
				<PatchFeatureBoundary>
					<ProductDemoSurface />
				</PatchFeatureBoundary>
			</AppProvider>
			<DeskLockOverlay />
		</ServerRuntime>
	);
}
