import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Battle CP Lobby | Join or Create a Game",
  description: "Join or create a competitive programming battle game. Customize difficulty, time limits, and challenge settings for real-time multiplayer Battleship + coding.",
};

export default function LobbyLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
