import { test, expect } from '@playwright/test';

test.describe('navigate to guides page', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto("/");
    });

    test("can navigate to guides from home page", async ({ page }) => {
        await page.getByRole('link', { name: "Start Hodling" }).click();
        await expect(page).toHaveURL(/\/guides/);
    });


    test("can navigate to guides from home page header", async ({ page }) => {
        await page.getByRole("link", { name: "Guides", exact: true }).click()
        await expect(page).toHaveURL(/\/guides/);
    });
});

test("all 3 guide cards are visible", async ({ page }) => {
    await page.goto('/guides');
    await expect(page.getByRole("button", { name: "basic" })).toBeVisible();
    await expect(page.getByRole("button", { name: "intermediate" })).toBeVisible();
    await expect(page.getByRole("button", { name: "advanced" })).toBeVisible();
});

test("can click card + platform cards appear", async ({ page }) => {
    await page.goto("/guides");
    await page.getByRole("button", { name: "basic" }).click();
    await expect(page.getByRole("button", { name: "Android" })).toBeVisible();
});

test("clicking on android loads guide + can pick wallet", async ({ page }) => {
    await page.goto("/guides");
    await page.getByRole("button", { name: "basic" }).click();
    await page.getByRole("button", { name: "Android" }).click();
    await expect(page).toHaveURL(/\/android/);
    await expect(page.getByRole("heading", { name: "Basic Android Self-Custody Guide" })).toBeVisible();
    await page.getByRole("button", { name: "Blue Wallet" }).click();
    await expect(page).toHaveURL(/\/android\/blue/);
});

test("blue wallet server functions return correct data", async ({ request }) => {
    const response = await request.post("/api/faq", {
        form: {
            faq_name: "bluewallet"
        }
    });
    const resText = await response.json()
    expect(resText).toHaveLength(5);
    expect(resText[0].title).toContain("bluewallet-quick-setup")
});
