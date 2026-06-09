import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "ForYield x Stellar - Soroban Vault (Testnet)",
  description:
    "MiCA-regulated DeFi yield vault on Stellar Soroban - testnet demo",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
