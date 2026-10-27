import type { Metadata, Viewport } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "ForYield Vault, built on Stellar (testnet)",
  description:
    "DeFi yield vault built for EU regulatory requirements, on Stellar Soroban - testnet demo",
};

export const viewport: Viewport = {
  themeColor: "#111a17",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <head>
        {/* Satoshi par l'API Fontshare (cf. globals.css). La CSP de
            render.yaml ne pose ni style-src ni font-src : rien a ouvrir. */}
        <link rel="preconnect" href="https://api.fontshare.com" />
        <link rel="preconnect" href="https://cdn.fontshare.com" crossOrigin="" />
        <link
          rel="stylesheet"
          href="https://api.fontshare.com/v2/css?f[]=satoshi@400,500,700&display=swap"
        />
      </head>
      <body>{children}</body>
    </html>
  );
}
