// Jazz v2 alpha schema — placeholder.
//
// TODO: implement with final jazz-tools@2.x API once stable.
// Intended shape:
//   PlayerNote { playerId, text, flagged, createdAt }
//   AdminPrefs { watchList: number[], notes: PlayerNote[] }

export type PlayerNote = {
  playerId: number;
  text: string;
  flagged: boolean;
  createdAt: number;
};

export type AdminPrefs = {
  watchList: number[];
  notes: PlayerNote[];
};
