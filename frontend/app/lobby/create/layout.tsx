import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Create a Game | Battle CP",
  description: "Create your own Battle CP game. Set difficulty level, time limits, heat thresholds, and veto settings. Invite opponents to join your competitive programming naval battle.",
  alternates: {
    canonical: `${process.env.NEXT_PUBLIC_APP_URL || "https://battle-cp.vercel.app"}/lobby/create`,
  },
};

export default function CreateLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
