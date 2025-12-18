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
  description: "A real-time strategy game blending Battleship mechanics with competitive programming problems.",
};

import { SoundProvider } from "@/context/SoundContext";
import { MusicProvider } from "@/context/MusicContext";

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
      </body>
    </html>
  );
}
