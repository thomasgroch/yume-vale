// Jazz v2 alpha integration — placeholder.
//
// jazz-tools@alpha (2.x) exports a new API incompatible with the 0.x schema.
// The actual exports are: JazzProvider, JazzClientProvider, createJazzClient,
// useDb, useJazzClient, useLocalFirstAuth, useAuthState, useSession, useAll.
//
// TODO: wire up once the v2 API stabilizes.
// The live game data (WebSocket) and core dashboard work without Jazz.
// Jazz will add: persistent admin notes, watchlist sync between sessions.

export const JAZZ_PEER =
  process.env.NEXT_PUBLIC_JAZZ_PEER ??
  "wss://cloud.jazz.tools/?key=yume-admin-dev";
