import React from "react";
import { Badge, Button, Group, Modal, Stack, Text } from "@mantine/core";
import { AcpPermissionRecord } from "../api";
import {
  NOTION_MODAL_CLASSNAMES,
  NOTION_MODAL_OVERLAY_PROPS,
} from "../ui/floating_surfaces";
import { SurfaceCard } from "../ui/primitives";

export type PermissionModalProps = {
  permissions: AcpPermissionRecord[];
  permissionBusy: string | null;
  onRespond: (agentId: string, permissionId: string, optionId?: string) => void;
  withinPortal?: boolean;
};

export function PermissionModal({
  permissions,
  permissionBusy,
  onRespond,
  withinPortal = true,
}: PermissionModalProps) {
  return (
    <Modal
      opened
      onClose={() => {}}
      withCloseButton={false}
      closeOnClickOutside={false}
      closeOnEscape={false}
      withinPortal={withinPortal}
      classNames={NOTION_MODAL_CLASSNAMES}
      title={
        <Group gap="xs">
          <Text fw={600}>Permission Requests</Text>
          <Badge variant="light">{permissions.length}</Badge>
        </Group>
      }
      size="lg"
      radius="md"
      overlayProps={NOTION_MODAL_OVERLAY_PROPS}
    >
      <Stack gap="sm">
        {permissions.map((perm) => {
          const toolCall = perm.tool_call as {
            title?: string;
            tool_call_id?: string;
          } | null;
          const title =
            toolCall?.title ?? perm.tool_call_id ?? "Permission Request";
          return (
            <SurfaceCard key={perm.id} className="rounded-md border-notion-border p-3">
              <Group justify="space-between" align="center">
                <Text fw={600}>{title}</Text>
                <Badge variant="outline" size="sm">
                  {perm.status}
                </Badge>
              </Group>
              <Group gap="xs" mt="sm" wrap="wrap">
                {perm.options.map((opt, idx) => {
                  const optionId = opt.option_id.trim();
                  return (
                    <Button
                      key={optionId || `${perm.id}-${idx}`}
                      size="xs"
                      disabled={permissionBusy === perm.id || !optionId}
                      onClick={() =>
                        onRespond(perm.agent_id, perm.id, optionId)
                      }
                    >
                      {opt.name}
                    </Button>
                  );
                })}
                <Button
                  size="xs"
                  variant="default"
                  disabled={permissionBusy === perm.id}
                  onClick={() => onRespond(perm.agent_id, perm.id)}
                >
                  Cancel
                </Button>
              </Group>
            </SurfaceCard>
          );
        })}
      </Stack>
    </Modal>
  );
}
