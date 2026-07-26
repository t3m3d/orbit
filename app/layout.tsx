import type { Metadata, Viewport } from "next";
import { headers } from "next/headers";
import "./globals.css";

export const viewport: Viewport = {
  themeColor: "#191919",
  colorScheme: "light",
  width: "device-width",
  initialScale: 1,
};

export async function generateMetadata(): Promise<Metadata> {
  const requestHeaders = await headers();
  const host = requestHeaders.get("x-forwarded-host") ?? requestHeaders.get("host") ?? "localhost:3000";
  const protocol = requestHeaders.get("x-forwarded-proto") ?? (host.includes("localhost") ? "http" : "https");
  const origin = `${protocol}://${host}`;

  return {
    title: "Orbit — Your transfers, everywhere",
    description: "A calm, universal torrent hub for managing downloads across desktop, mobile, and the web.",
    applicationName: "Orbit",
    manifest: "/manifest.webmanifest",
    appleWebApp: {
      capable: true,
      statusBarStyle: "black-translucent",
      title: "Orbit",
    },
    formatDetection: { telephone: false },
    openGraph: {
      title: "Orbit — Your transfers, everywhere",
      description: "One beautifully simple torrent queue across every screen.",
      type: "website",
      url: origin,
      siteName: "Orbit",
      images: [{ url: `${origin}/og.png`, width: 1200, height: 630, alt: "Orbit universal torrent hub" }],
    },
    twitter: {
      card: "summary_large_image",
      title: "Orbit — Your transfers, everywhere",
      description: "One beautifully simple torrent queue across every screen.",
      images: [`${origin}/og.png`],
    },
  };
}

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>
        {children}
        <script dangerouslySetInnerHTML={{ __html: "if('serviceWorker' in navigator){window.addEventListener('load',()=>navigator.serviceWorker.register('/sw.js').catch(()=>{}))}" }} />
      </body>
    </html>
  );
}