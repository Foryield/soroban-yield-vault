import { NETWORK_LABEL } from "@/lib/stellar";

// En-tete co-brande, commun aux deux pages. Regles de la brand policy SDF
// (stellar.org/brand-policy) : logo Stellar officiel, inaltere, en version
// blanche sur fond sombre ; jamais plus visible que le lockup ForYield ; et
// « Built on Stellar » plutot qu'un « ForYield x Stellar » qui laisserait lire
// un partenariat. Les deux mentions qu'elle exige sont dans le pied de page.
export function TopBar({ sub, wallet }: { sub?: string; wallet?: string | null }) {
  return (
    <header className="topbar">
      <div className="cobrand">
        <a href="https://www.for-yield.com" aria-label="ForYield">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img className="fy-logo" src="/brand/foryield-lockup-cream.svg" alt="ForYield" />
        </a>
        <span className="divider" aria-hidden="true" />
        <span className="built-on">
          Built on
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/brand/stellar-logo-white.svg" alt="Stellar" />
        </span>
        {sub && <span className="topbar-sub">{sub}</span>}
      </div>
      <div className="topbar-right">
        {wallet && (
          <span className="wallet-chip" title={wallet}>
            {wallet.slice(0, 4)}...{wallet.slice(-4)}
          </span>
        )}
        <span className="badge">{NETWORK_LABEL}</span>
      </div>
    </header>
  );
}

export function Footer() {
  return (
    <footer className="footer">
      <p>
        Testnet demo, Stellar Community Fund build. Testnet tokens only, with no value.{" "}
        <a href="https://www.for-yield.com">for-yield.com</a>
      </p>
      <p>
        ForYield is not an authorised crypto-asset service provider; this page is not an offer
        of a financial service.
      </p>
      <p>
        &ldquo;Stellar&rdquo; is a trademark of the Stellar Development Foundation. All rights
        reserved. This is independent software, not affiliated with, sponsored or endorsed by
        the Stellar Development Foundation.
      </p>
    </footer>
  );
}
