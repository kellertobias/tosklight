import { test, expect } from "../apps/control-ui/e2e/bench/fixtures";
import { ApiDriver, type Session } from "../apps/control-ui/e2e/bench/api";

type ClientSummary = {
  client_id: string;
  name: string;
  connected: boolean;
  last_connected_at: string | null;
  can_remove: boolean;
  desk: { id: string; name: string; osc_alias: string; columns: number; rows: number; buttons: number; playback_layout?: unknown };
};

test.describe("docs/plans/Done/22-client-history-and-removal.DONE.md", () => {
  test.describe.configure({ mode: "serial" });

  test("CLIENT-001 @restart › client presence, history, removal, and clean re-registration remain desk-local", async ({ api, bench, desk, page, show }) => {
    test.setTimeout(90_000);
    const clientB = crypto.randomUUID();
    const sessionB = await createSession(bench.baseUrl, clientB);
    const clientBApi = new ApiDriver(bench.baseUrl);
    clientBApi.session = sessionB;

    let clients = await clientSummaries(api);
    const observer = clients.find((client) => client.client_id === api.session!.client_id)!;
    let historical = clients.find((client) => client.client_id === clientB)!;
    expect(observer.connected).toBe(true);
    expect(historical.connected).toBe(true);
    expect(observer.can_remove).toBe(false);
    expect(historical.can_remove).toBe(false);
    expect(clients.slice(0, 2).every((client) => client.connected)).toBe(true);

    await expect(api.request("DELETE", `/api/v1/clients/${historical.desk.id}`)).rejects.toThrow(/409.*actively connected/);
    await expect(api.request("DELETE", `/api/v1/clients/${observer.desk.id}`)).rejects.toThrow(/409.*current client/);

    await clientBApi.request("DELETE", `/api/v1/sessions/${sessionB.session_id}`);
    clients = await clientSummaries(api);
    historical = clients.find((client) => client.client_id === clientB)!;
    expect(historical.connected).toBe(false);
    expect(historical.can_remove).toBe(true);
    expect(historical.last_connected_at).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    const disconnectedAt = historical.last_connected_at;
    const historicalDesk = historical.desk;

    await bench.stopServerGracefully(api.session!.token);
    await bench.startServer();
    await api.login();
    clients = await clientSummaries(api);
    historical = clients.find((client) => client.client_id === clientB)!;
    expect(historical).toMatchObject({ connected: false, last_connected_at: disconnectedAt, desk: { id: historicalDesk.id } });

    await desk.open(bench.baseUrl);
    await page.getByRole("button", { name: /Open show menu/ }).click();
    await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
    await page.locator(".setup-window nav").getByRole("button", { name: "Screens & playback", exact: true }).click();
    await page.getByRole("button", { name: "Choose default screen", exact: true }).click();
    const chooser = page.getByRole("dialog", { name: "Choose default screen" });
    await expect(chooser.getByRole("heading", { name: "Connected clients", exact: true })).toBeVisible();
    await expect(chooser.getByRole("heading", { name: "Disconnected clients", exact: true })).toBeVisible();
    const historicalRow = chooser.getByRole("article").filter({ hasText: clientB });
    await expect(historicalRow).toContainText("Disconnected");
    await expect(historicalRow).toContainText("Last connected");
    await historicalRow.getByRole("button", { name: "Remove client" }).click();
    const confirmation = page.getByRole("alertdialog", { name: new RegExp(`Remove client ${escapeRegex(historical.name)}`) });
    await expect(confirmation).toContainText("Portable shows, users, optional screens, other clients, and installation-wide configuration will not change");
    await confirmation.getByRole("button", { name: "Cancel" }).click();
    await chooser.getByRole("button", { name: "Close default screen chooser" }).click();

    const reconnected = await createSession(bench.baseUrl, clientB, historicalDesk.id);
    expect(reconnected.desk.id).toBe(historicalDesk.id);
    clients = await clientSummaries(api);
    expect(clients.filter((client) => client.client_id === clientB)).toHaveLength(1);
    expect(clients.find((client) => client.client_id === clientB)?.connected).toBe(true);
    const reconnectedApi = new ApiDriver(bench.baseUrl);
    reconnectedApi.session = reconnected;
    await reconnectedApi.request("DELETE", `/api/v1/sessions/${reconnected.session_id}`);

    const showBefore = await api.request<any>("GET", `/api/v1/shows/${show.id}/objects/group`, undefined, false);
    const usersBefore = (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).users;
    await api.request("DELETE", `/api/v1/clients/${historicalDesk.id}`);
    clients = await clientSummaries(api);
    expect(clients.some((client) => client.client_id === clientB)).toBe(false);
    expect(clients.some((client) => client.client_id === api.session!.client_id)).toBe(true);
    expect((await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).users).toEqual(usersBefore);
    expect(await api.request<any>("GET", `/api/v1/shows/${show.id}/objects/group`, undefined, false)).toEqual(showBefore);

    const fresh = await createSession(bench.baseUrl, clientB, historicalDesk.id);
    expect(fresh.desk.id).not.toBe(historicalDesk.id);
    expect(fresh.desk).toMatchObject({ columns: 8, rows: 1, buttons: 3 });
    expect(fresh.desk.playback_layout ?? null).toBeNull();
    const afterReRegistration = await clientSummaries(api);
    expect(afterReRegistration.filter((client) => client.client_id === clientB)).toHaveLength(1);
    expect(afterReRegistration.find((client) => client.client_id === clientB)?.desk.id).toBe(fresh.desk.id);
  });
});

async function createSession(baseUrl: string, clientId: string, deskId: string | null = null): Promise<Session> {
  const response = await fetch(`${baseUrl}/api/v1/sessions`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ username: "Operator", client_id: clientId, desk_id: deskId }),
  });
  expect(response.ok).toBe(true);
  return response.json() as Promise<Session>;
}

async function clientSummaries(api: ApiDriver): Promise<ClientSummary[]> {
  return (await api.request<{ clients: ClientSummary[] }>("GET", "/api/v1/bootstrap", undefined, false)).clients;
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
