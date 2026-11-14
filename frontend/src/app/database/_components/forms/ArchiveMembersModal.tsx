import {
  Alert,
  Button,
  Group,
  Modal,
  ScrollArea,
  Stack,
  Table,
  Text,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useRef } from "react";

import type { ArchiveMembersModalProps } from "@/app/database/_components/shared/types";

import { useArchiveMembersMutation } from "@/lib/api/queries/members";

export default function ArchiveMembersModal({
  members,
  onClose,
}: ArchiveMembersModalProps) {
  const archive = useArchiveMembersMutation();
  const submitting = useRef(false);

  const close = () => {
    if (!submitting.current) onClose();
  };

  const count = (total: number) =>
    `${total} ${total === 1 ? "member" : "members"}`;
  const failures = new Map(
    archive.data?.failed.map(({ member_id, error }) => [member_id, error]),
  );

  return (
    <Modal
      opened
      onClose={close}
      title="Are you sure?"
      centered
      size="lg"
      closeOnClickOutside={!archive.isPending}
      closeOnEscape={!archive.isPending}
      withCloseButton={!archive.isPending}
    >
      <Stack>
        <Text>
          <Text span fw={700}>
            {count(members.length)}
          </Text>{" "}
          will be removed from the active members list. Their records will be
          archived and retained.
        </Text>
        <ScrollArea.Autosize mah={320}>
          <Table>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>ID</Table.Th>
                <Table.Th>Name</Table.Th>
                <Table.Th>Email</Table.Th>
                {failures.size > 0 && <Table.Th>Status</Table.Th>}
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {members.map((member) => (
                <Table.Tr key={member.member_id}>
                  <Table.Td>{member.member_id}</Table.Td>
                  <Table.Td>
                    {member.first_name} {member.last_name}
                  </Table.Td>
                  <Table.Td>{member.email}</Table.Td>
                  {failures.size > 0 && (
                    <Table.Td c="red">
                      {failures.get(member.member_id)}
                    </Table.Td>
                  )}
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </ScrollArea.Autosize>
        {(archive.error || failures.size > 0) && (
          <Alert color="red" role="alert">
            {archive.error?.message ??
              `${count(failures.size)} could not be archived.`}
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

              archive.mutate(
                members.map((member) => member.member_id),
                {
                  onSuccess: ({ archived, failed }) => {
                    if (archived.length)
                      notifications.show({
                        message: `${count(archived.length)} archived.`,
                        color: "green",
                      });
                    if (!failed.length) onClose();
                  },
                  onSettled: () => {
                    submitting.current = false;
                  },
                },
              );
            }}
          >
            Confirm
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
