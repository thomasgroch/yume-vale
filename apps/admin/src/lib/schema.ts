// Jazz v2 alpha data model for the admin panel.
// Stores collaborative admin state: notes and watchlist.
// Live game data (positions, connections) comes from the game server WS,
// not from Jazz — Jazz only persists what admins add themselves.

import { co, CoList, CoMap } from "jazz-tools";

export class PlayerNote extends CoMap {
  playerId = co.number();
  text = co.string();
  flagged = co.boolean();
  createdAt = co.number(); // unix ms
}

export class PlayerNoteList extends CoList.Of(co.ref(PlayerNote)) {}

export class AdminPrefs extends CoMap {
  // player IDs pinned to top of the list
  watchList = co.json<number[]>();
  notes = co.ref(PlayerNoteList);
}
