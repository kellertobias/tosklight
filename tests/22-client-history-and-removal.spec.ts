import { test, expect } from "./bench/core/fixtures";
import { ApiDriver, type Session } from "./bench/core/api";

type ClientSummary = {
	client_id: string;
	name: string;
	connected: boolean;
	last_connected_at: string | null;
	can_remove: boolean;
	desk: {
		id: string;
		name: string;
		columns: number;
		rows: number;
		buttons: number;
		playback_layout?: unknown;
	};
};

test.describe("docs/plans/Done/22-client-history-and-removal.DONE.md", () => {
	test.describe.configure({ mode: "serial" });

	test("CLIENT-001 @ui @restart › a second client leaves the single-user desk working", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(90_000);
		// The desk is heading for a single user with however many browsers on it, so what matters
		// here is that a second client neither disturbs this desk nor survives a restart as a
		// stale one. The wider multi-user history and removal behaviour is deliberately not pinned.
		const clientB = crypto.randomUUID();
		const sessionB = await createSession(bench.baseUrl, clientB);
		const clientBApi = new ApiDriver(bench.baseUrl);
		clientBApi.session = sessionB;

		let clients = await clientSummaries(api);
		const observer = clients.find(
			(client) => client.client_id === api.session!.client_id,
		)!;
		expect(observer.connected).toBe(true);
		expect(
			clients.find((client) => client.client_id === clientB)?.connected,
		).toBe(true);

		await clientBApi.request(
			"DELETE",
			`/api/v2/sessions/${sessionB.session_id}`,
		);
		clients = await clientSummaries(api);
		expect(
			clients.find((client) => client.client_id === clientB)?.connected,
		).toBe(false);
		expect(
			clients.find((client) => client.client_id === api.session!.client_id)
				?.connected,
		).toBe(true);

		await bench.stopServerGracefully(api.session!.token);
		await bench.startServer();
		await api.login();

		await desk.open(bench.baseUrl);
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page
			.getByRole("button", { name: "Enter Setup", exact: true })
			.click();
		await page
			.locator(".setup-window nav")
			.getByRole("button", { name: "Screens & playback", exact: true })
			.click();
		await page
			.getByRole("button", { name: "Known windows", exact: true })
			.click();
		const chooser = page.getByRole("dialog", { name: "Known windows" });
		await expect(
			chooser.getByRole("heading", { name: "Connected windows", exact: true }),
		).toBeVisible();
		// A client is a window, not a desk, so the list identifies windows rather than desk
		// configurations to choose between. This window is marked, and single-client mode — on by
		// default — has already dropped the disconnected one.
		await expect(chooser.getByText("This window")).toHaveCount(1);
		await expect(
			chooser.getByRole("article").filter({ hasText: clientB }),
		).toHaveCount(0);
		await chooser
			.getByRole("button", { name: "Close known windows" })
			.click();

		// Forgetting a disconnected window drops its record and nothing else. Every window shares
		// the one desk, so this must not take the desk — and with it the page, playback selection
		// and desk lock — away from the windows still standing at it.
		const clientC = crypto.randomUUID();
		const sessionC = await createSession(bench.baseUrl, clientC);
		const clientCApi = new ApiDriver(bench.baseUrl);
		clientCApi.session = sessionC;
		await clientCApi.request(
			"DELETE",
			`/api/v2/sessions/${sessionC.session_id}`,
		);
		const deskId = api.session!.desk.id;
		await removeClient(api, deskId, clientC);
		clients = await clientSummaries(api);
		expect(clients.some((client) => client.client_id === clientC)).toBe(false);
		expect(clients.some((client) => client.desk.id === deskId)).toBe(true);
		expect(
			clients.find((client) => client.client_id === api.session!.client_id)
				?.connected,
		).toBe(true);
	});
});

async function createSession(
	baseUrl: string,
	clientId: string,
	deskId: string | null = null,
): Promise<Session> {
	const response = await fetch(`${baseUrl}/api/v2/sessions`, {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify({
			username: "Operator",
			client_id: clientId,
			desk_id: deskId,
		}),
	});
	expect(response.ok).toBe(true);
	return response.json() as Promise<Session>;
}

async function clientSummaries(api: ApiDriver): Promise<ClientSummary[]> {
	return (
		await api.request<{ clients: ClientSummary[] }>(
			"GET",
			"/api/v2/bootstrap",
			undefined,
			false,
		)
	).clients;
}

async function removeClient(
	api: ApiDriver,
	deskId: string,
	clientId: string,
	requestId = crypto.randomUUID(),
) {
	return api.request<any>("POST", `/api/v2/control-desks/${deskId}/actions`, {
		request_id: requestId,
		action: { type: "remove_client", client_id: clientId },
	});
}

function escapeRegex(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
