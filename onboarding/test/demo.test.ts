import { afterEach, describe, it, expect, vi } from "vitest";
import type { Server } from "node:http";
import { createDemoServer, type DemoBricks } from "../src/demo/server.js";

function makeBricks() {
  return {
    onboardWallet: vi.fn().mockResolvedValue({ walletId: "wa-1", address: "GABC" }),
    deposit: vi.fn().mockResolvedValue({ txHash: "abc123", ledger: 42, successful: true }),
  } satisfies DemoBricks;
}

let server: Server | undefined;

function listen(bricks: DemoBricks): Promise<string> {
  return new Promise((resolve) => {
    server = createDemoServer(bricks).listen(0, () => {
      const address = server!.address();
      if (address === null || typeof address === "string") throw new Error("no ephemeral port");
      resolve(`http://127.0.0.1:${address.port}`);
    });
  });
}

afterEach(() => {
  server?.close();
  server = undefined;
});

describe("demo server", () => {
  it("POST /api/onboard provisions a wallet for the given email", async () => {
    const bricks = makeBricks();
    const base = await listen(bricks);

    const response = await fetch(`${base}/api/onboard`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email: "user@example.com" }),
    });

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ walletId: "wa-1", address: "GABC" });
    expect(bricks.onboardWallet).toHaveBeenCalledWith("user@example.com");
  });

  it("POST /api/onboard without email returns 400 with an error", async () => {
    const bricks = makeBricks();
    const base = await listen(bricks);

    const response = await fetch(`${base}/api/onboard`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
    });

    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({ error: expect.stringContaining("email") });
    expect(bricks.onboardWallet).not.toHaveBeenCalled();
  });

  it("POST /api/deposit submits a fixed 0.1 XLM deposit and returns the explorer link", async () => {
    const bricks = makeBricks();
    const base = await listen(bricks);

    const response = await fetch(`${base}/api/deposit`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ walletId: "wa-1", address: "GABC" }),
    });

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({
      txHash: "abc123",
      ledger: 42,
      successful: true,
      explorerUrl: "https://stellar.expert/explorer/testnet/tx/abc123",
    });
    // Amount is fixed server-side: the page never chooses how much to move.
    expect(bricks.deposit).toHaveBeenCalledWith("GABC", "wa-1", 1_000_000n);
  });

  it("POST /api/deposit without walletId or address returns 400", async () => {
    const bricks = makeBricks();
    const base = await listen(bricks);

    for (const body of [{ address: "GABC" }, { walletId: "wa-1" }]) {
      const response = await fetch(`${base}/api/deposit`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      expect(response.status).toBe(400);
      expect(await response.json()).toEqual({ error: expect.any(String) });
    }
    expect(bricks.deposit).not.toHaveBeenCalled();
  });

  it("returns 500 with the message only when a brick fails", async () => {
    const bricks = makeBricks();
    bricks.onboardWallet.mockRejectedValue(new Error("DFNS unreachable"));
    const base = await listen(bricks);

    const response = await fetch(`${base}/api/onboard`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email: "user@example.com" }),
    });

    expect(response.status).toBe(500);
    expect(await response.json()).toEqual({ error: "DFNS unreachable" });
  });

  it("GET / serves the demo page", async () => {
    const base = await listen(makeBricks());

    const response = await fetch(`${base}/`);
    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toContain("text/html");
    const html = await response.text();
    expect(html).toContain("For Yield");
    expect(html).toContain("/api/onboard");
  });
});
