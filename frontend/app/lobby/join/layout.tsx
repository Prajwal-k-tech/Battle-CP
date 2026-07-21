import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Join a Game | Battle CP",
  description: "Enter a game code to join an active Battle CP match. Enter your Codeforces handle and start competing in real-time multiplayer Battleship with coding challenges.",
  alternates: {
    canonical: `${process.env.NEXT_PUBLIC_APP_URL || "https://battle-cp.duckdns.org"}/lobby/join`,
  },
};

export default function JoinLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
