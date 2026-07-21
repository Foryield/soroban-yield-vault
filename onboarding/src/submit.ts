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
  let lastErrorStatus: number | undefined;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if (attempt > 0) await new Promise((resolve) => setTimeout(resolve, delayMs));
    const response = await fetchImpl(`${horizonUrl}/transactions/${txHash}`);
    if (response.status === 200) {
      const body = await response.json();
      return { successful: body.successful === true, ledger: Number(body.ledger) };
    }
    // 404 = not yet included (expected while polling); anything else is a
    // Horizon error worth surfacing if we exhaust our attempts.
    if (response.status !== 404) lastErrorStatus = response.status;
  }
  if (lastErrorStatus !== undefined) {
    throw new Error(
      `transaction ${txHash} not confirmed after ${attempts} attempts (last status=${lastErrorStatus})`,
    );
  }
  throw new Error(`transaction ${txHash} not found on Horizon after ${attempts} attempts`);
}
