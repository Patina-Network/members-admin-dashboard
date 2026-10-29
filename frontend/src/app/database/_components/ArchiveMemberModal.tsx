import { Alert, Button, Group, Modal, Stack, Text } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useRef } from "react";

import type { ArchiveMemberModalProps } from "@/app/database/_components/types";

import { useArchiveMemberMutation } from "@/lib/api/queries/members";

export default function ArchiveMemberModal({
  member,
  onClose,
}: ArchiveMemberModalProps) {
  const archive = useArchiveMemberMutation();
  const submitting = useRef(false);
  const close = () => {
    if (!submitting.current) onClose();
  };
  return (
    <Modal
      opened
      onClose={close}
      title="Are you sure?"
      centered
      closeOnClickOutside={!archive.isPending}
      closeOnEscape={!archive.isPending}
      withCloseButton={!archive.isPending}
    >
      <Stack>
        <Text>
          <Text span fw={700}>
            {member.first_name} {member.last_name}
          </Text>{" "}
          will be removed from the active members list. Their record will be
          archived and retained.
        </Text>
        <Stack gap="xs">
          <Text>Email: {member.email}</Text>
          <Text>Member ID: {member.member_id}</Text>
        </Stack>
        {archive.error && (
          <Alert color="red" role="alert">
            {archive.error.message}
          </Alert>
        )}
        <Group justify="flex-end">
          <Button
            variant="default"
            disabled={archive.isPending}
            onClick={close}
          >
            Cancel
          </Button>
          <Button
            color="red"
            loading={archive.isPending}
            disabled={archive.isPending}
            onClick={() => {
              if (submitting.current) return;
              submitting.current = true;
              archive.mutate(member.member_id, {
                onSuccess: () => {
                  notifications.show({
                    message: "Member archived.",
                    color: "green",
                  });
                  onClose();
                },
                onSettled: () => {
                  submitting.current = false;
                },
              });
            }}
          >
            Archive member
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
