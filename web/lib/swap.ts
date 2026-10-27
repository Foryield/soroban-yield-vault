// --- Echange D4 -----------------------------------------------------------------
// Reequilibrage USDC -> EURC par le SwapRouter du livrable 4, qui sert l'echange
// sur le pool Aquarius USDC / EURC et garantit au moins `min_out`, sinon tout
// revert. Preuves : docs/evidence/d4-dex-routing.md.
//
// La cotation simule `swap_exact_in` sur le routeur lui-meme, avec un minimum
// symbolique : elle rend ce que l'echange servirait, frais compris, sans rien
// soumettre. Le minimum signe retranche ensuite la marge de glissement.

import { Address, Asset, Contract, nativeToScVal } from "@stellar/stellar-sdk";
import { IS_TESTNET, PASSPHRASE } from "./network";
import { invokeContract, readContract } from "./stellar";
import { VAULTS, type AssetDescriptor } from "./vaults";

// Routeur mono-venue du 28/08/2026. Sans adresse, le panneau ne s'affiche pas.
export const SWAP_ROUTER_ID =
  process.env.NEXT_PUBLIC_SWAP_ROUTER_ID ||
  (IS_TESTNET ? "CCQJWT73HTZUVLM2UUPUA5VR53Z5MTHCRZDVF5RODH3ORMVALNQQY6EA" : "");

/// Marge de glissement des demonstrations D4 : cotation moins 1 %.
export const SLIPPAGE_BPS = 100n;

/// Paire du pool D4 : l'USDC de Blend (onglet USDC) vers l'EURC de Circle
/// (onglet EURC), les actifs memes des deux vaults.
export const SWAP_PAIR = (() => {
  const usdc = VAULTS.find((v) => v.key === "usdc")?.asset;
  const eurc = VAULTS.find((v) => v.key === "eurc")?.asset;
  return SWAP_ROUTER_ID && usdc?.kind === "classic" && eurc?.kind === "classic"
    ? { from: usdc, to: eurc }
    : null;
})();

export type SwapQuote = {
  amountIn: bigint;
  /// Sortie servie au prix du moment.
  amountOut: bigint;
  /// Minimum signe : en dessous, le routeur revert.
  minOut: bigint;
  /// Frais du pool, preleves sur l'actif d'entree.
  fee: bigint;
};

type RawSwapResult = { amount_out: bigint; fee: bigint };

function sacId(asset: Extract<AssetDescriptor, { kind: "classic" }>): string {
  return new Asset(asset.code, asset.issuer).contractId(PASSPHRASE);
}

function swapArgs(from: string, amountIn: bigint, minOut: bigint) {
  if (!SWAP_PAIR) throw new Error("No swap router configured for this network");
  return [
    new Address(from).toScVal(),
    new Address(sacId(SWAP_PAIR.from)).toScVal(),
    new Address(sacId(SWAP_PAIR.to)).toScVal(),
    nativeToScVal(amountIn, { type: "i128" }),
    nativeToScVal(minOut, { type: "i128" }),
  ];
}

// Erreurs du routeur (contracts/router/src/lib.rs, RouterError) rendues
// lisibles ; le reste remonte tel quel.
function routerError(e: unknown): Error {
  const message = e instanceof Error ? e.message : String(e);
  if (message.includes("Error(Contract, #6)")) {
    return new Error("The Aquarius pool cannot serve this swap now: not enough liquidity.");
  }
  if (message.includes("Error(Contract, #7)")) {
    return new Error("The price moved beyond the 1 % margin. Get a new quote.");
  }
  if (message.includes("Error(Contract, #5)")) {
    return new Error("This pair is not registered in the swap router.");
  }
  return e instanceof Error ? e : new Error(message);
}

export async function quoteSwap(from: string, amountIn: bigint): Promise<SwapQuote> {
  try {
    const raw = (await readContract(
      SWAP_ROUTER_ID,
      "swap_exact_in",
      ...swapArgs(from, amountIn, 1n),
    )) as RawSwapResult;
    const amountOut = BigInt(raw.amount_out);
    return {
      amountIn,
      amountOut,
      minOut: (amountOut * (10_000n - SLIPPAGE_BPS)) / 10_000n,
      fee: BigInt(raw.fee),
    };
  } catch (e) {
    throw routerError(e);
  }
}

export type SwapReceipt = { hash: string; amountOut: bigint; fee: bigint };

export async function executeSwap(from: string, quote: SwapQuote): Promise<SwapReceipt> {
  try {
    const { hash, result } = await invokeContract(
      from,
      new Contract(SWAP_ROUTER_ID),
      "swap_exact_in",
      ...swapArgs(from, quote.amountIn, quote.minOut),
    );
    const served = result as RawSwapResult;
    return { hash, amountOut: BigInt(served.amount_out), fee: BigInt(served.fee) };
  } catch (e) {
    throw routerError(e);
  }
}
