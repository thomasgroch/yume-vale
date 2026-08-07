// Jazz v2 alpha — local-first sync for collaborative admin state.
// Syncs AdminPrefs (notes, watchlist) between browser sessions via Jazz Cloud.
//
// Jazz Cloud peer: the free tier is enough for a small admin panel.
// Set NEXT_PUBLIC_JAZZ_PEER to self-host (see https://jazz.tools/docs/self-hosting).

import { createJazzReactApp, DemoAuth } from "jazz-react";
import { AdminPrefs } from "./schema";

export const { JazzProvider, useAccount, useCoState } = createJazzReactApp({
  AccountSchema: AdminPrefs,
});

export const jazzPeer =
  process.env.NEXT_PUBLIC_JAZZ_PEER ??
  "wss://cloud.jazz.tools/?key=yume-admin-dev";
