import { Box, Button, SimpleGrid, Table, Text } from "@mantine/core";

import type { MemberTableRowProps } from "@/app/database/_components/shared/types";

import { roleLabels } from "@/app/database/_components/shared/memberRoles";
import { useMemberDetailsQuery } from "@/lib/api/queries/members";

function display(value: string | null | undefined) {
  return value || "—";
}

function MemberProfile({ memberId }: { memberId: number }) {
  const query = useMemberDetailsQuery(memberId);

  return (
    <Box mt="xs">
      <Text size="sm" fw={500}>
        Profile notes:
      </Text>
      {query.isPending ?
        <Text size="sm">Loading…</Text>
      : query.isError ?
        <Text size="sm" c="red">
          {query.error.message}
        </Text>
      : <Text
          size="sm"
          style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}
        >
          {display(query.data.profile)}
        </Text>
      }
    </Box>
  );
}

export default function MemberTableRow({
  member,
  expanded,
  onToggle,
  onEdit,
  onArchive,
}: MemberTableRowProps) {
  return (
    <>
      <Table.Tr>
        <Table.Td>{member.member_id}</Table.Td>
        <Table.Td>{member.email}</Table.Td>
        <Table.Td>{member.first_name}</Table.Td>
        <Table.Td>{member.last_name}</Table.Td>
        <Table.Td>{display(member.discord_username)}</Table.Td>
        <Table.Td>{member.role ? roleLabels[member.role] : "—"}</Table.Td>
        <Table.Td>
          <Button
            variant="subtle"
            size="xs"
            aria-label={`${expanded ? "Hide" : "Show"} additional info for member ${member.member_id}`}
            aria-controls={`member-details-${member.member_id}`}
            aria-expanded={expanded}
            onClick={onToggle}
          >
            {expanded ? "See less" : "See more"}
          </Button>
        </Table.Td>
        <Table.Td>
          <Button
            variant="light"
            size="xs"
            aria-label={`Edit member ${member.member_id}`}
            onClick={() => onEdit(member)}
          >
            Edit
          </Button>
          <Button
            variant="light"
            color="red"
            size="xs"
            ml="xs"
            aria-label={`Archive member ${member.member_id}`}
            onClick={() => onArchive(member)}
          >
            Archive
          </Button>
        </Table.Td>
      </Table.Tr>
      {expanded && (
        <Table.Tr id={`member-details-${member.member_id}`}>
          <Table.Td colSpan={8}>
            <Text fw={500} mb="xs">
              Additional info
            </Text>
            <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="xs">
              <Text size="sm">
                <Text span fw={500}>
                  Joined at:{" "}
                </Text>
                {member.joined_at ?
                  new Date(member.joined_at).toLocaleString()
                : "—"}
              </Text>
              <Text size="sm">
                <Text span fw={500}>
                  Discord ID:{" "}
                </Text>
                {display(member.discord_id)}
              </Text>
              <Text size="sm">
                <Text span fw={500}>
                  LinkedIn:{" "}
                </Text>
                {display(member.linkedin_username)}
              </Text>
              <Text size="sm">
                <Text span fw={500}>
                  Instagram:{" "}
                </Text>
                {display(member.instagram_username)}
              </Text>
              <Text size="sm">
                <Text span fw={500}>
                  Career:{" "}
                </Text>
                {display(member.career)}
              </Text>
              <Text size="sm">
                <Text span fw={500}>
                  Industry:{" "}
                </Text>
                {display(member.industry)}
              </Text>
            </SimpleGrid>
            <MemberProfile memberId={member.member_id} />
          </Table.Td>
        </Table.Tr>
      )}
    </>
  );
}
