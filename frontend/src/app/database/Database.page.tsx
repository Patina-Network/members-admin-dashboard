import {
  Alert,
  Box,
  Button,
  Group,
  Pagination,
  Table,
  Text,
  Title,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { useState } from "react";

import type { Member } from "@/lib/api/generated/types.gen";

import AddMembersModal from "@/app/database/_components/AddMembersModal";
import EditMemberModal from "@/app/database/_components/EditMemberModal";
import { useMembersQuery } from "@/lib/api/queries/members";

export default function DatabasePage() {
  const [opened, { open, close }] = useDisclosure(false);
  const members = useMembersQuery();
  const [editing, setEditing] = useState<Member | null>(null);
  return (
    <>
      <Group justify="space-between" mb="md">
        <Box>
          <Title order={2}>Database</Title>
          <Text c="dimmed">
            {members.data?.total != null ?
              `${members.data.total} ${members.data.total === 1 ? "member" : "members"}`
            : "— members"}
          </Text>
        </Box>
        <Button type="button" onClick={open}>
          Add
        </Button>
      </Group>
      <AddMembersModal opened={opened} onClose={close} />
      {editing && (
        <EditMemberModal
          key={editing.member_id}
          member={editing}
          onClose={() => setEditing(null)}
        />
      )}
      {members.isError && (
        <Alert
          color="red"
          role="alert"
          mb="md"
          title={
            members.data ?
              "Unable to refresh members"
            : "Unable to load members"
          }
        >
          <Text>
            {members.data ?
              "Showing previously loaded members. Please try again."
            : "Please try again to load the member list."}
          </Text>
          <Button
            mt="sm"
            variant="light"
            onClick={() => void members.refetch()}
            loading={members.isFetching}
          >
            Retry
          </Button>
        </Alert>
      )}
      {members.isFetching && members.data && (
        <Text role="status" mb="sm">
          Refreshing members…
        </Text>
      )}
      <Box mah="70vh" style={{ overflow: "auto" }}>
        <Table striped withRowBorders={false} verticalSpacing="md">
          <Table.Thead>
            <Table.Tr>
              <Table.Th>ID</Table.Th>
              <Table.Th>Email</Table.Th>
              <Table.Th>First name</Table.Th>
              <Table.Th>Last name</Table.Th>
              <Table.Th>Joined at</Table.Th>
              <Table.Th>Discord</Table.Th>
              <Table.Th>Actions</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {members.data?.members.map((member) => (
              <Table.Tr key={member.member_id}>
                <Table.Td>{member.member_id}</Table.Td>
                <Table.Td>{member.email}</Table.Td>
                <Table.Td>{member.first_name}</Table.Td>
                <Table.Td>{member.last_name}</Table.Td>
                <Table.Td>
                  {member.joined_at ?
                    new Date(member.joined_at).toLocaleString()
                  : "—"}
                </Table.Td>
                <Table.Td>
                  {member.discord_username ?? member.discord_id ?? "—"}
                </Table.Td>
                <Table.Td>
                  <Button
                    variant="light"
                    size="xs"
                    aria-label={`Edit member ${member.member_id}`}
                    onClick={() => setEditing(member)}
                  >
                    Edit
                  </Button>
                </Table.Td>
              </Table.Tr>
            ))}
            {(!members.data || members.data.members.length === 0) && (
              <Table.Tr>
                <Table.Td colSpan={7}>
                  <Text fw={500} ta="center">
                    {members.isPending ?
                      "Loading members…"
                    : members.isError ?
                      "Member list unavailable"
                    : "Members will appear here"}
                  </Text>
                </Table.Td>
              </Table.Tr>
            )}
          </Table.Tbody>
        </Table>
      </Box>
      <Group justify="center" mt="md">
        <Pagination total={1} value={1} disabled />
      </Group>
    </>
  );
}
