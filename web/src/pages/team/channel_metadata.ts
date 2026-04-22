export type TeamChannelId = "all";

export type TeamChannelItem = {
  id: TeamChannelId;
  label: string;
  description: string;
};

export const DEFAULT_TEAM_CHANNEL_ITEMS: ReadonlyArray<TeamChannelItem> = [
  {
    id: "all",
    label: "# all",
    description: "Shared coordination lane for requests, updates, and cross-cutting discussion.",
  },
];
