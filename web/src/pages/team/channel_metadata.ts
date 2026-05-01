import type { TeamChannelRecord } from "../../api";

export const DEFAULT_TEAM_CHANNEL_ID = "all";

export type TeamChannelId = string;

export type TeamChannelItem = {
  id: TeamChannelId;
  label: string;
  description: string;
};

export const DEFAULT_TEAM_CHANNEL_ITEMS: ReadonlyArray<TeamChannelItem> = [
  {
    id: DEFAULT_TEAM_CHANNEL_ID,
    label: "# all",
    description: "Shared coordination lane for requests, updates, and cross-cutting discussion.",
  },
];

export function buildTeamChannelItems(
  channels: ReadonlyArray<TeamChannelRecord>
): ReadonlyArray<TeamChannelItem> {
  if (channels.length === 0) {
    return DEFAULT_TEAM_CHANNEL_ITEMS;
  }
  return [
    ...DEFAULT_TEAM_CHANNEL_ITEMS,
    ...channels.map((channel) => ({
      id: channel.channel_id,
      label: `# ${channel.channel_id}`,
      description:
        channel.description?.trim() || `Focused Team lane for ${channel.channel_id}.`,
    })),
  ];
}

export function describeTeamKanban(channelLabel: string): string {
  return `Canonical Kanban for coordinator-planned, system-managed Team tasks. Human task requests belong in ${channelLabel}.`;
}
