import { expect, type Page } from "@playwright/test";

export class DeskDriver {
  constructor(readonly page: Page) {}

  async open(baseUrl: string): Promise<void> {
    await this.page.goto(baseUrl);
    await expect(this.page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    await expect(this.page.locator(".connection-banner")).toBeHidden({ timeout: 10_000 });
  }

  async command(value: string, visibleValue = formatVisibleCommand(value)): Promise<void> {
    const command = this.page.getByLabel("Command line");
    await this.page.getByRole("button", { name: "ESC", exact: true }).click();
    const keys = value.trim().split(/\s+/).flatMap((token) => {
      if (token === "GROUP") return ["GRP"];
      if (/^\d+$/.test(token)) return [...token];
      return [token];
    });
    for (const key of keys) {
      await this.page.getByRole("button", { name: key, exact: true }).click();
    }
    await expect(command).toHaveValue(visibleValue);
    await this.page.getByRole("button", { name: "ENT", exact: true }).click();
  }

  async openFixtures(): Promise<void> {
    await this.page.getByRole("button", { name: "BUILT-INS" }).click();
    await this.page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
  }
}

function formatVisibleCommand(value: string): string {
  const tokens = value.trim().split(/\s+/);
  let scope: "fixture" | "group" = "fixture";
  let expectsSelectionNumber = true;
  return tokens.map((token) => {
    if (token === "GROUP") {
      scope = "group";
      expectsSelectionNumber = true;
      return null;
    }
    if (token === "+" || token === "-") {
      expectsSelectionNumber = true;
      return token;
    }
    if (token === "AT" || token === "THRU") {
      expectsSelectionNumber = token === "THRU";
      return token;
    }
    if (/^\d+$/.test(token) && expectsSelectionNumber) {
      expectsSelectionNumber = false;
      return `${scope === "group" ? "G" : "F"}${token}`;
    }
    return token;
  }).filter((token): token is string => token !== null).join(" ");
}
