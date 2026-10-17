export interface Points {
  skill: number;
  attribute: number;
  devotion: number;
}
export interface Installation {
  path: string;
  buildId: string | null;
  expansions: string[];
}
export interface Home {
  game: string | null;
  installations: Installation[];
  platform: string;
}
