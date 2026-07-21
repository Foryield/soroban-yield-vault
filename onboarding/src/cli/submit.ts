import { loadConfig } from "../config.js";
import { dfnsClient } from "../dfns.js";
import { loadDotenv } from "../env.js";
import { submitViaDfns, waitForInclusion } from "../submit.js";

loadDotenv();

const [walletId, hex] = process.argv.slice(2);
if (!walletId || !hex) {
  console.error("Usage: npm run submit -- <walletId> <signed-tx-hex>");
  process.exit(1);
}

try {
  const cfg = loadConfig();
  const client = dfnsClient(cfg);
  const { txHash } = await submitViaDfns(client, walletId, hex);
  const inclusion = await waitForInclusion(cfg.horizonUrl, txHash);
  console.log(JSON.stringify({ txHash, ledger: inclusion.ledger, successful: inclusion.successful }));
  if (!inclusion.successful) {
    console.error(`transaction ${txHash} included in ledger ${inclusion.ledger} but failed on-chain`);
    process.exit(2);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
