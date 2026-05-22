import { expect, test } from '@playwright/test';

test("Homepage load", async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveTitle("WE HODL BTC - Bitcoin Self-Custody Guides & Live Blockchain Analytics");
    await expect(page.getByRole("link", { name: "WE HODL BTC" })).toBeVisible();
});

