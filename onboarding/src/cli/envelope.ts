import { loadConfig } from "../config.js";
import { loadDotenv } from "../env.js";
import { buildInvocationHex, depositArgs, withdrawArgs } from "../envelope.js";

loadDotenv();

const [method, source, amountRaw] = process.argv.slice(2);
if ((method !== "deposit" && method !== "withdraw") || !source || !amountRaw) {
  console.error("Usage: npm run envelope -- <deposit|withdraw> <source-G-address> <amount-stroops>");
  process.exit(1);
}

try {
  const amount = BigInt(amountRaw);
  if (amount <= 0n) throw new Error(`amount must be a positive number of stroops, got ${amountRaw}`);
  const cfg = loadConfig();
  const args = method === "deposit" ? depositArgs(source, amount) : withdrawArgs(source, amount);
  const built = await buildInvocationHex({
    contractId: cfg.vaultContractId,
    method,
    args,
    source,
    rpcUrl: cfg.rpcUrl,
  });
  console.log(JSON.stringify(built));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
