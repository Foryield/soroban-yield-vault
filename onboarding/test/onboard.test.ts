import { describe, it, expect, vi } from "vitest";
import { onboard, type OnboardDeps } from "../src/onboard.js";

function makeDeps() {
  return {
    provision: vi.fn().mockResolvedValue({ walletId: "wa-1", address: "GABC" }),
    fund: vi.fn().mockResolvedValue(undefined),
    buildEnvelope: vi.fn().mockResolvedValue({ xdrBase64: "AA==", hex: "0xdead" }),
    submit: vi.fn().mockResolvedValue({ requestId: "tx-1", status: "Broadcasted", txHash: "abc" }),
    waitForInclusion: vi.fn().mockResolvedValue({ successful: true, ledger: 42 }),
  } satisfies OnboardDeps;
}

describe("onboard", () => {
  it("chains provision -> fund -> envelope -> submit -> inclusion", async () => {
    const deps = makeDeps();
    const summary = await onboard("user@example.com", 1_000_000n, deps);

    expect(deps.provision).toHaveBeenCalledWith("user@example.com");
    expect(deps.fund).toHaveBeenCalledWith("GABC");
    expect(deps.buildEnvelope).toHaveBeenCalledWith("GABC", 1_000_000n);
    expect(deps.submit).toHaveBeenCalledWith("wa-1", "0xdead");
    expect(deps.waitForInclusion).toHaveBeenCalledWith("abc");

    // The envelope carries a sequence number: it must be built after funding
    // (the account only exists on-chain then), right before submission.
    expect(deps.buildEnvelope).toHaveBeenCalledAfter(deps.fund);
    const order = [deps.provision, deps.fund, deps.buildEnvelope, deps.submit, deps.waitForInclusion].map(
      (fn) => fn.mock.invocationCallOrder[0],
    );
    expect(order).toEqual([...order].sort((a, b) => a - b));

    expect(summary).toEqual({
      email: "user@example.com",
      walletId: "wa-1",
      address: "GABC",
      txHash: "abc",
      ledger: 42,
      successful: true,
    });
  });

  it("resolves with successful: false when inclusion reports an on-chain failure", async () => {
    const deps = makeDeps();
    deps.waitForInclusion.mockResolvedValue({ successful: false, ledger: 43 });

    // The orchestrator reports; the CLI decides the exit code.
    const summary = await onboard("user@example.com", 1_000_000n, deps);
    expect(summary).toMatchObject({ successful: false, ledger: 43, txHash: "abc" });
  });
});
