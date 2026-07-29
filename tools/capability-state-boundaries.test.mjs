import assert from "node:assert/strict";
import test from "node:test";
import {
	appStateRawFields,
	capabilityStateBoundaryFailures,
} from "./capability-state-boundaries.mjs";

const emptyDebt = () => ({
	appStateFields: {},
	adapterAccess: {},
	publicApis: {},
	resourceEscapes: {},
	taskOwnership: {},
});

test("detects raw lock, store, and registry fields directly on AppState", () => {
	const source = `
pub(super) struct AppState {
    pub(super) service: PlaybackService,
    pub(super) desk: Arc<Mutex<DeskStore>>,
    pub(super) programmers: ProgrammerRegistry,
}`;
	assert.deepEqual([...appStateRawFields(source).keys()], [
		"desk: Arc<Mutex<DeskStore>>",
		"programmers: ProgrammerRegistry",
	]);
});

test("allows typed adapter ports and capability-owned resource internals", () => {
	const entries = [
		{
			path: "crates/light/adapters/headless/src/runtime/state.rs",
			source: "pub(super) struct AppState { pub(super) playback: PlaybackService, }",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/playback_http.rs",
			source:
				"async fn action(State(state): State<AppState>) { state.playback.execute(command); }",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/capabilities/show/resource.rs",
			source:
				"pub(super) struct Resource { store: Mutex<ShowStore> } impl Resource { fn read(&self) { self.store.lock(); } }",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/capabilities/show/repository.rs",
			source:
				"pub(super) struct Repository { store: ShowStore } impl Repository { fn open() { ShowStore::open(\"show\"); } }",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/startup_state.rs",
			source:
				"fn compose() { let desk = DeskStore::open(\"desk\"); let fixtures = FixtureLibrary::open(\"fixtures\"); }",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/capabilities/output/supervisor.rs",
			source:
				"fn start() { let cancel = CancellationToken::new(); tokio::spawn(async move {}); }",
		},
	];
	assert.deepEqual(
		capabilityStateBoundaryFailures(entries, { debt: emptyDebt() }),
		[],
	);
});

test("composition roots may construct raw owners but may not expose or lock them", () => {
	const entries = [
		{
			path: "crates/light/adapters/headless/src/runtime/state.rs",
			source: "pub(super) struct AppState {}",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/startup_state.rs",
			source: `
fn compose() { let store = DeskStore::open("desk"); }
pub(super) fn leak() -> DeskStore { todo!() }
fn lock(state: &AppState) { state.installation.desk.lock(); }
`,
		},
	];
	const failures = capabilityStateBoundaryFailures(entries, {
		debt: emptyDebt(),
	});
	assert(!failures.some((failure) => failure.includes("raw-owner:DeskStore")));
	assert(failures.some((failure) => failure.includes("|state-lock")));
	assert(failures.some((failure) => failure.includes("lock-bearing public API")));
});

test("capability composition may inject concrete services but resources may not expose them", () => {
	const allowed = [
		{
			path: "crates/light/adapters/headless/src/runtime/state.rs",
			source: "pub(super) struct AppState {}",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/capabilities/show/resource.rs",
			source:
				"struct Resource { service: ActiveShowService } impl Resource { pub(super) fn new(service: ActiveShowService) -> Self { Self { service } } }",
		},
	];
	assert.deepEqual(
		capabilityStateBoundaryFailures(allowed, { debt: emptyDebt() }),
		[],
	);

	const closureEscape = [
		allowed[0],
		{
			path: "crates/light/adapters/headless/src/runtime/capabilities/show/resource.rs",
			source:
				"struct Resource { service: ActiveShowService } impl Resource { pub(super) fn with_service<R>(&self, operation: impl FnOnce(&ActiveShowService) -> R) -> R { operation(&self.service) } }",
		},
	];
	assert(
		capabilityStateBoundaryFailures(closureEscape, {
			debt: emptyDebt(),
		}).some((failure) =>
			failure.includes("concrete-active-show-signature"),
		),
	);

	const exposed = [
		allowed[0],
		{
			path: "crates/light/adapters/headless/src/runtime/capabilities/show/resource.rs",
			source:
				"struct Resource { pub(super) service: ActiveShowService } impl Resource { pub(super) fn service(&self) -> ActiveShowService { self.service.clone() } }",
		},
	];
	const failures = capabilityStateBoundaryFailures(exposed, {
		debt: emptyDebt(),
	});
	assert(
		failures.some((failure) =>
			failure.includes("concrete-resource-field"),
		),
	);
	assert(
		failures.some((failure) =>
			failure.includes("concrete-resource-return"),
		),
	);

	const exposedOutput = [
		allowed[0],
		{
			path: "crates/light/adapters/headless/src/runtime/capabilities/output/resource.rs",
			source: `
struct Resource {
    pub(super) engine: Arc<Engine>,
    pub(super) rate: Arc<AtomicU16>,
    pub(super) network: Option<Arc<NetworkOutput>>,
    pub(super) manual_clock: Option<Arc<ManualClock>>,
    pub(super) test_clock_lock: Arc<tokio::sync::Mutex<()>>,
}
`,
		},
	];
	const outputFailures = capabilityStateBoundaryFailures(exposedOutput, {
		debt: emptyDebt(),
	});
	assert.equal(
		outputFailures.filter((failure) =>
			failure.includes("concrete-resource-field"),
		).length,
		5,
	);
});

test("rejects new adapter raw locks, stores, lock APIs, and task ownership", () => {
	const entries = [
		{
			path: "crates/light/adapters/headless/src/runtime/state.rs",
			source: "pub(super) struct AppState {}",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/show_http.rs",
			source: `
use light_show::ShowStore;
pub(super) fn store(state: &AppState) -> Arc<Mutex<ShowStore>> {
    let guard = state.desk.lock();
    tokio::spawn(async move {});
    Arc::new(Mutex::new(ShowStore::open("show").unwrap()))
}`,
		},
	];
	const failures = capabilityStateBoundaryFailures(entries, {
		debt: emptyDebt(),
	});
	assert(failures.some((failure) => failure.includes("|state-lock")));
	assert(failures.some((failure) => failure.includes("raw-owner:ShowStore")));
	assert(failures.some((failure) => failure.includes("lock-bearing public API")));
	assert(failures.some((failure) => failure.includes("tokio-spawn")));
});

test("rejects public capability resource fields and Deref service escapes", () => {
	const entries = [
		{
			path:
				"crates/light/adapters/headless/src/runtime/capability_resources.rs",
			source: `
pub(super) struct PlaybackResource {
    pub(super) service: PlaybackService,
    telemetry: PlaybackTelemetrySampler,
}
impl std::ops::Deref for PlaybackResource {
    type Target = PlaybackService;
    fn deref(&self) -> &Self::Target { &self.service }
}`,
		},
	];
	const failures = capabilityStateBoundaryFailures(entries, {
		debt: emptyDebt(),
	});
	assert(
		failures.some((failure) =>
			failure.includes("public-field:PlaybackResource.service"),
		),
	);
	assert(
		failures.some((failure) =>
			failure.includes("deref:PlaybackResource->PlaybackService"),
		),
	);
});

test("exact debt inventory rejects additions and becomes stale after cleanup", () => {
	const path =
		"crates/light/adapters/headless/src/runtime/legacy_adapter.rs";
	const debt = emptyDebt();
	debt.adapterAccess[`${path}|state-lock`] = 1;
	const oneLock = [
		{
			path: "crates/light/adapters/headless/src/runtime/state.rs",
			source: "pub(super) struct AppState {}",
		},
		{ path, source: "fn read(state: &AppState) { state.desk.lock(); }" },
	];
	assert.deepEqual(capabilityStateBoundaryFailures(oneLock, { debt }), []);

	const addition = [
		...oneLock.slice(0, 1),
		{
			path,
			source:
				"fn read(state: &AppState) { state.desk.lock(); state.desk.lock(); }",
		},
	];
	assert(
		capabilityStateBoundaryFailures(addition, { debt }).some((failure) =>
			failure.includes("ownership debt grew"),
		),
	);
	assert(
		capabilityStateBoundaryFailures(oneLock.slice(0, 1), { debt }).some(
			(failure) => failure.includes("stale ownership debt"),
		),
	);
});

test("tests do not create production ownership debt", () => {
	const entries = [
		{
			path: "crates/light/adapters/headless/src/runtime/state.rs",
			source: "pub(super) struct AppState {}",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/tests/raw_state_tests.rs",
			source:
				"fn harness(state: &AppState) { state.desk.lock(); tokio::spawn(async {}); }",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/worker.rs",
			source:
				"fn production() {} #[cfg(test)] mod tests { fn harness() { let cancel = CancellationToken::new(); tokio::spawn(async {}); } }",
		},
	];
	assert.deepEqual(
		capabilityStateBoundaryFailures(entries, { debt: emptyDebt() }),
		[],
	);
});

test("bounded request and benchmark-local task owners are not application lifecycle debt", () => {
	const entries = [
		{
			path: "crates/light/adapters/headless/src/runtime/state.rs",
			source: "pub(super) struct AppState {}",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/test_bench.rs",
			source:
				"async fn bounded() { let cancel = CancellationToken::new(); run(cancel).await; }",
		},
		{
			path: "apps/light-headless/src/bin/light_benchmark/loopback.rs",
			source:
				"struct LocalReceiver { receiver: Option<JoinHandle<()>> } impl Drop for LocalReceiver { fn drop(&mut self) { self.receiver.take().unwrap().join().unwrap(); } }",
		},
		{
			path: "crates/light/adapters/headless/src/runtime/visualization_transport.rs",
			source:
				"async fn socket() { let writer = tokio::spawn(async {}); writer.await.unwrap(); }",
		},
	];
	assert.deepEqual(
		capabilityStateBoundaryFailures(entries, { debt: emptyDebt() }),
		[],
	);

	const unownedRuntimeTask = [
		...entries.slice(0, 1),
		{
			path: "crates/light/adapters/headless/src/runtime/other.rs",
			source: "fn detached() { tokio::spawn(async {}); }",
		},
	];
	assert(
		capabilityStateBoundaryFailures(unownedRuntimeTask, {
			debt: emptyDebt(),
		}).some((failure) => failure.includes("tokio-spawn")),
	);
});
