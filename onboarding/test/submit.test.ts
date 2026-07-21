import { describe, it, expect, vi } from "vitest";
import { submitViaDfns, waitForInclusion } from "../src/submit.js";

describe("submitViaDfns", () => {
  it("returns the tx hash when DFNS broadcasts", async () => {
    const client = {
      wallets: {
        broadcastTransaction: vi.fn().mockResolvedValue({
          id: "tx-1",
          status: "Broadcasted",
          txHash: "abc123",
        }),
      },
    };
    const result = await submitViaDfns(client as never, "wa-1", "0xdead");
    expect(client.wallets.broadcastTransaction).toHaveBeenCalledWith({
      walletId: "wa-1",
      body: { kind: "Transaction", transaction: "0xdead" },
    });
    expect(result).toEqual({ requestId: "tx-1", status: "Broadcasted", txHash: "abc123" });
  });

  it("throws a clear error when a policy holds the transaction", async () => {
    const client = {
      wallets: {
        broadcastTransaction: vi.fn().mockResolvedValue({
          id: "tx-2",
          status: "PendingApproval",
        }),
      },
    };
    await expect(submitViaDfns(client as never, "wa-1", "0xdead")).rejects.toThrow(/PendingApproval/);
  });

  it("includes the DFNS reason when the transaction fails", async () => {
    const client = {
      wallets: {
        broadcastTransaction: vi.fn().mockResolvedValue({
          id: "tx-3",
          status: "Failed",
          reason: "insufficient funds for fee",
        }),
      },
    };
    await expect(submitViaDfns(client as never, "wa-1", "0xdead")).rejects.toThrow(
      /insufficient funds for fee/,
    );
  });
});

describe("waitForInclusion", () => {
  it("resolves once Horizon reports the transaction successful", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce({ status: 404 })
      .mockResolvedValueOnce({ status: 200, ok: true, json: async () => ({ successful: true, ledger: 42 }) });
    const result = await waitForInclusion("https://horizon", "abc123", {
      fetchImpl: fetchImpl as never,
      delayMs: 0,
    });
    expect(result).toEqual({ successful: true, ledger: 42 });
  });

  it("coerces a string ledger to a number", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue({ status: 200, ok: true, json: async () => ({ successful: true, ledger: "7" }) });
    const result = await waitForInclusion("https://horizon", "abc123", { fetchImpl: fetchImpl as never });
    expect(result).toEqual({ successful: true, ledger: 7 });
  });

  it("throws when the transaction never appears on Horizon", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({ status: 404 });
    await expect(
      waitForInclusion("https://horizon", "abc123", { fetchImpl: fetchImpl as never, delayMs: 0, attempts: 2 }),
    ).rejects.toThrow(/not found on Horizon after 2 attempts/);
    expect(fetchImpl).toHaveBeenCalledTimes(2);
  });

  it("reports the last non-404 status when Horizon errors", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce({ status: 500 })
      .mockResolvedValueOnce({ status: 404 });
    await expect(
      waitForInclusion("https://horizon", "abc123", { fetchImpl: fetchImpl as never, delayMs: 0, attempts: 2 }),
    ).rejects.toThrow(/not confirmed after 2 attempts \(last status=500\)/);
  });
});
