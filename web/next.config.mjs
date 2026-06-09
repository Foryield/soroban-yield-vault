/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Export statique : la page est 100% client-side, aucun rendu serveur.
  // Produit un dossier `out/` deployable comme site statique (Render, etc.).
  output: "export",
  images: { unoptimized: true },
};

export default nextConfig;
