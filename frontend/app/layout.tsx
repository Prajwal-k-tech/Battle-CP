import type { Metadata } from "next";
import { Inter, JetBrains_Mono, Space_Grotesk, Press_Start_2P } from "next/font/google"; // 1. Import
import { ThemeProvider } from "next-themes";
import { Toaster } from "sonner";
import "./globals.css";
import { cn } from "@/lib/utils";

const inter = Inter({
  subsets: ["latin"],
  variable: "--font-inter",
  display: "swap",
});

const spaceGrotesk = Space_Grotesk({
  subsets: ["latin"],
  variable: "--font-space-grotesk",
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  variable: "--font-jetbrains-mono",
  display: "swap",
});

const pressStart2P = Press_Start_2P({ // 2. Configure
  weight: "400",
  subsets: ["latin"],
  variable: "--font-press-start-2p",
  display: "swap",
});

export const metadata: Metadata = {
  title: "Battle CP | Competitive Programming Naval Combat",
  description: "A real-time strategy game blending Battleship mechanics with competitive programming. Solve Codeforces problems to cool down your weapons and defeat your opponent.",
  keywords: ["Competitive Programming", "Codeforces", "Battleship", "Coding Game", "Programming Strategy", "Battle CP", "Algorithms Game"],
  authors: [{ name: "oGhostyyy" }],
  openGraph: {
    title: "Battle CP | Competitive Programming Naval Combat",
    description: "Battleship meets Competitive Programming. Solve problems to win the naval war.",
    url: process.env.NEXT_PUBLIC_APP_URL || "https://battle-cp.vercel.app",
    siteName: "Battle CP",
    images: [
      {
        url: "/og-image.svg",
        width: 1200,
        height: 630,
        alt: "Battle CP Gameplay",
      },
    ],
    locale: "en_US",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "Battle CP | Competitive Programming Naval Combat",
    description: "Solve Codeforces problems to win a game of Battleship.",
    images: ["/og-image.svg"],
    creator: "@oGhostyyy",
  },
  alternates: {
    canonical: process.env.NEXT_PUBLIC_APP_URL || "https://battle-cp.vercel.app",
  },
  robots: {
    index: true,
    follow: true,
  },
};

import { SoundProvider } from "@/context/SoundContext";
import { MusicProvider } from "@/context/MusicContext";
import { Analytics } from "@vercel/analytics/next";
import { SpeedInsights } from "@vercel/speed-insights/next";

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body
        className={cn(
          "min-h-screen bg-background font-sans antialiased",
          inter.variable,
          spaceGrotesk.variable,
          jetbrainsMono.variable,
          pressStart2P.variable
        )}
      >
        <ThemeProvider
          attribute="class"
          defaultTheme="dark"
          enableSystem={false}
          forcedTheme="dark"
          disableTransitionOnChange
        >
          <SoundProvider>
            <MusicProvider>
              {children}
              <Toaster
                position="bottom-right"
                toastOptions={{
                  className: "border border-white/10 bg-black text-white",
                }}
              />
            </MusicProvider>
          </SoundProvider>
        </ThemeProvider>
        <Analytics />
        <SpeedInsights />
      </body>
    </html>
  );
}
