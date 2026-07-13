import { expect, type Page } from "@playwright/test";

export class DeskDriver {
  constructor(readonly page: Page) {}

  async open(baseUrl: string): Promise<void> {
    await this.page.goto(baseUrl);
    await expect(this.page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    await expect(this.page.locator(".connection-banner")).toBeHidden({ timeout: 10_000 });
  }

  async command(value: string): Promise<void> {
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
    await expect(command).toHaveValue(value);
    await command.press("Enter");
  }

  async openFixtures(): Promise<void> {
    await this.page.getByRole("button", { name: "BUILT-INS" }).click();
    await this.page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
  }
}
