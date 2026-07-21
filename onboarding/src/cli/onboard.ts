import { loadConfig } from "../config.js";
import { dfnsClient } from "../dfns.js";
import { buildInvocationHex, depositArgs } from "../envelope.js";
import { onboard } from "../onboard.js";
import { provisionWallet, fundWithFriendbot } from "../provision.js";
import { submitViaDfns, waitForInclusion } from "../submit.js";

const [email, stroopsRaw] = process.argv.slice(2);
if (!email || !stroopsRaw) {
  console.error("Usage: npm run onboard -- <email> <amount-stroops>");
  process.exit(1);
}

try {
  const stroops = BigInt(stroopsRaw);
  const cfg = loadConfig();
  const client = dfnsClient(cfg);
  const summary = await onboard(email, stroops, {
    provision: (name) => provisionWallet(client, name),
    fund: fundWithFriendbot,
    buildEnvelope: (source, amount) =>
      buildInvocationHex({
        contractId: cfg.vaultContractId,
        method: "deposit",
        args: depositArgs(source, amount),
        source,
        rpcUrl: cfg.rpcUrl,
      }),
    submit: (walletId, hex) => submitViaDfns(client, walletId, hex),
    waitForInclusion: (txHash) => waitForInclusion(cfg.horizonUrl, txHash),
  });
  console.log(JSON.stringify(summary));
  if (!summary.successful) {
    console.error(`transaction ${summary.txHash} included in ledger ${summary.ledger} but failed on-chain`);
    process.exit(2);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
