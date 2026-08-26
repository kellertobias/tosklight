import { expect, test } from "./bench/core/fixtures";

/**
 * The released-Desk performance harness talks to the desk over the v2 API. It only ever runs on a
 * Linux runner against a released bundle, so nothing here proves it end to end — this walks the
 * same request sequence against the bench desk so a route it can no longer call is caught before
 * a twenty-minute CI round trip.
 */
test("PERF-HARNESS @api the routes the performance harness calls still answer", async ({
	api,
	bench,
}) => {
	const raw = async (method: string, path: string, body?: unknown) => {
		const response = await fetch(`${bench.baseUrl}${path}`, {
			method,
			headers: {
				...(body === undefined ? {} : { "content-type": "application/json" }),
				authorization: `Bearer ${api.session?.token}`,
			},
			body: body === undefined ? undefined : JSON.stringify(body),
		});
		return response;
	};

	// A session carries no user: v2 resolves the desk from the client, and the harness used to
	// look up an enabled operator that the bootstrap has never exposed.
	const session = await raw("POST", "/api/v2/sessions", { desk_id: null });
	expect(session.status, "a session needs no username").toBe(200);
	expect(await session.json()).toMatchObject({ session_id: expect.any(String) });

	const bootstrap = await api.request<Record<string, unknown>>(
		"GET",
		"/api/v2/bootstrap",
	);
	expect(bootstrap.active_show ?? null).not.toBeUndefined();
	// The desk deliberately has no users to discover, which is what broke the harness.
	expect(bootstrap.users).toBeUndefined();

	const scheduler = await raw("POST", "/api/v2/configuration/update", {
		request_id: crypto.randomUUID(),
		patch: { frame_rate_hz: 40 },
	});
	expect(scheduler.status, "the harness pins the output rate").toBe(200);

	const patch = await api.request<Record<string, any>>("GET", "/api/v2/patch");
	expect(patch.patch_revision).toEqual(expect.any(Number));

	const profiles = await api.request<{ profiles: unknown[] }>(
		"GET",
		"/api/v2/fixture-library/profiles",
	);
	expect(Array.isArray(profiles.profiles)).toBe(true);

	// The measurement window reads frame counters and the rate bands from this one route.
	const diagnostics = await api.request<{ output: Record<string, unknown> }>(
		"GET",
		"/api/v2/diagnostics/performance",
	);
	expect(diagnostics.output.frames_sent).toEqual(expect.any(Number));
	expect(Array.isArray(diagnostics.output.frame_rate_band_counts)).toBe(true);
	expect(Array.isArray(diagnostics.output.frame_rate_band_bounds_hz)).toBe(true);

});
