import type { Metadata } from "next";
import ClientGame from "../ClientGame";

// Dynamic metadata for game pages (server component)
export async function generateMetadata({
  params,
}: {
  params: Promise<{ gameId: string }>;
}): Promise<Metadata> {
  const { gameId } = await params;
  const baseUrl = process.env.NEXT_PUBLIC_APP_URL || "https://battle-cp.vercel.app";

  return {
    title: `Battle CP - Game ${gameId.slice(0, 8)}`,
    description: `Join an active Battle CP game. Test your competitive programming skills in real-time naval combat. Game ID: ${gameId}`,
        robots: {
            index: false,
            follow: false,
        },
    openGraph: {
      title: `Battle CP - Game ${gameId.slice(0, 8)}`,
      description: "Real-time multiplayer Battleship + Competitive Programming game",
      type: "website",
      url: `${baseUrl}/game/${gameId}`,
      images: [{ url: "/og-image.svg", width: 1200, height: 630 }],
    },
    twitter: {
      card: "summary_large_image",
      title: `Battle CP - Game ${gameId.slice(0, 8)}`,
      description: "Playing Battle CP - competitive programming meets Battleship",
      images: ["/og-image.svg"],
    },
    alternates: {
      canonical: `${baseUrl}/game/${gameId}`,
    },
  };
}

export default function GamePage({ params }: { params: Promise<{ gameId: string }> }) {
    return <ClientGame params={params} />;
}
