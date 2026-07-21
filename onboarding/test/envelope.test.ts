import { describe, it, expect, vi } from "vitest";
import { Account, Address } from "@stellar/stellar-sdk";
import { depositArgs, withdrawArgs, buildInvocationHex, type InvocationRequest } from "../src/envelope.js";

const G_ADDRESS = "GCUKCTOCRTLX52H2BWAA4EL5TE5PCECUSKFOG7BALI2TPFZRLIHJC5RS";
const CONTRACT_ID = "CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6";

function fakeAccount(): Account {
  return new Account(G_ADDRESS, "1");
}

function opts(): InvocationRequest {
  return {
    contractId: CONTRACT_ID,
    method: "deposit",
    args: depositArgs(G_ADDRESS, 1_000_000n),
    source: G_ADDRESS,
    rpcUrl: "https://soroban-testnet.stellar.org",
  };
}

describe("depositArgs", () => {
  it("encodes from + stroops as ScVal [Address, i128]", () => {
    const [from, amount] = depositArgs(G_ADDRESS, 1_000_000n);
    expect(Address.fromScVal(from).toString()).toBe(G_ADDRESS);
    expect(amount.switch().name).toBe("scvI128");
  });
});

describe("withdrawArgs", () => {
  it("encodes from + shares as ScVal [Address, i128]", () => {
    const [from, shares] = withdrawArgs(G_ADDRESS, 500n);
    expect(Address.fromScVal(from).toString()).toBe(G_ADDRESS);
    expect(shares.switch().name).toBe("scvI128");
  });
});

describe("buildInvocationHex", () => {
  it("throws with the simulation error when simulate fails", async () => {
    const server = {
      getAccount: vi.fn().mockResolvedValue(fakeAccount()),
      simulateTransaction: vi.fn().mockResolvedValue({ error: "host invocation failed" }),
    };
    await expect(buildInvocationHex(opts(), server as never)).rejects.toThrow(/host invocation failed/);
  });
});
