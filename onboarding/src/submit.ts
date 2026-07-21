import type { DfnsApiClient } from "@dfns/sdk";

export type SubmitResult = { requestId: string; status: string; txHash: string };

export async function submitViaDfns(client: DfnsApiClient, walletId: string, hex: string): Promise<SubmitResult> {
  const response = await client.wallets.broadcastTransaction({
    walletId,
    body: { kind: "Transaction", transaction: hex },
  });
  // The SDK types txHash as optional even when Broadcasted; anything else means
  // the tx is held (policy approval), failed, or rejected — surface it loudly.
  if (response.status !== "Broadcasted" || !response.txHash) {
    const reason = response.reason ? ` reason=${response.reason}` : "";
    throw new Error(`DFNS did not broadcast: status=${response.status}${reason} (request ${response.id})`);
  }
  return { requestId: response.id, status: response.status, txHash: response.txHash };
}

export type InclusionResult = { successful: boolean; ledger: number };

// DFNS "Broadcasted" is necessary but not sufficient: inclusion on-chain is the
// truth, so poll Horizon until the transaction shows up (404 until included).
export async function waitForInclusion(
  horizonUrl: string,
  txHash: string,
  options: { fetchImpl?: typeof fetch; delayMs?: number; attempts?: number } = {},
): Promise<InclusionResult> {
  const { fetchImpl = fetch, delayMs = 2000, attempts = 15 } = options;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const response = await fetchImpl(`${horizonUrl}/transactions/${txHash}`);
    if (response.status === 200) {
      const body = await response.json();
      return { successful: body.successful === true, ledger: body.ledger };
    }
    await new Promise((resolve) => setTimeout(resolve, delayMs));
  }
  throw new Error(`transaction ${txHash} not found on Horizon after ${attempts} attempts`);
}
