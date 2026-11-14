import {
  Alert,
  Box,
  Button,
  Checkbox,
  Group,
  Pagination,
  Skeleton,
  Stack,
  Table,
  Text,
  TextInput,
  Title,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { useState } from "react";

import type { Member } from "@/lib/api/generated/types.gen";

import AddMembersModal from "@/app/database/_components/forms/AddMembersModal";
import ArchiveMembersModal from "@/app/database/_components/forms/ArchiveMembersModal";
import EditMemberModal from "@/app/database/_components/forms/EditMemberModal";
import {
  membersPerPage,
  useMemberTable,
} from "@/app/database/_components/shared/hook";
import MembersTableSkeleton from "@/app/database/_components/table/MembersTableSkeleton";
import MemberTableRow from "@/app/database/_components/table/MemberTableRow";
import RoleFilter from "@/app/database/_components/table/RoleFilter";

export default function DatabasePage() {
  const [opened, { open, close }] = useDisclosure(false);
  const {
    members,
    search,
    setSearch,
    roles,
    setRoles,
    setPage,
    currentPage,
    pageCount,
    visibleMembers,
    matchingCount,
    hasSearch,
    showCreatedMember,
  } = useMemberTable();
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [editing, setEditing] = useState<Member | null>(null);
  const [archivingIds, setArchivingIds] = useState<number[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());

  const allMembers = members.data?.members ?? [];
  const archivingMembers = allMembers.filter((member) =>
    archivingIds.includes(member.member_id),
  );
  const selectedMembers = allMembers.filter((member) =>
    selectedIds.has(member.member_id),
  );
  const selectedOnPage = visibleMembers.filter((member) =>
    selectedIds.has(member.member_id),
  ).length;
  const pageSelected =
    visibleMembers.length > 0 && selectedOnPage === visibleMembers.length;

  const select = (targets: Member[], selected: boolean) =>
    setSelectedIds((current) => {
      const next = new Set(current);
      for (const { member_id } of targets) {
        if (selected) next.add(member_id);
        else next.delete(member_id);
      }
      return next;
    });

  return (
    <>
      <Group justify="space-between" mb="md">
        <Box>
          <Title order={2}>Database</Title>
          <Skeleton
            visible={members.isPending}
            width={members.isPending ? 110 : "auto"}
          >
            <Text c="dimmed">
              {members.data?.total != null ?
                `${members.data.total} ${members.data.total === 1 ? "member" : "members"}`
              : "— members"}
            </Text>
          </Skeleton>
        </Box>
        <Stack gap="xs">
          <Button type="button" onClick={open}>
            Add
          </Button>
          <RoleFilter
            roles={roles}
            onChange={(value) => {
              setRoles(value);
              setExpandedId(null);
            }}
          />
        </Stack>
      </Group>
      <TextInput
        label="Search by first name"
        placeholder="First name"
        type="search"
        size="sm"
        maw={280}
        value={search}
        onChange={(event) => {
          setSearch(event.currentTarget.value);
          setExpandedId(null);
        }}
        mb="md"
        rightSectionWidth={search ? 70 : undefined}
        rightSection={
          search ?
            <Button
              variant="subtle"
              size="compact-xs"
              onClick={() => {
                setSearch("");
                setExpandedId(null);
              }}
            >
              Clear
            </Button>
          : undefined
        }
      />
      {(hasSearch || roles.length > 0) && members.data && (
        <Text size="sm" c="dimmed" mb="sm" role="status">
          {matchingCount}{" "}
          {matchingCount === 1 ? "matching member" : "matching members"}
        </Text>
      )}
      <AddMembersModal
        opened={opened}
        onClose={close}
        onCreated={showCreatedMember}
      />
      {editing && (
        <EditMemberModal
          key={editing.member_id}
          member={editing}
          onClose={() => setEditing(null)}
        />
      )}
      {archivingMembers.length > 0 && (
        <ArchiveMembersModal
          members={archivingMembers}
          onClose={() => setArchivingIds([])}
        />
      )}
      {selectedMembers.length > 0 && (
        <Group mb="sm" gap="sm">
          <Text size="sm" role="status">
            {selectedMembers.length}{" "}
            {selectedMembers.length === 1 ? "member" : "members"} selected
          </Text>
          <Button
            color="red"
            size="xs"
            onClick={() => setArchivingIds([...selectedIds])}
          >
            Archive All
          </Button>
          <Button
            variant="subtle"
            size="xs"
            onClick={() => setSelectedIds(new Set())}
          >
            Clear selection
          </Button>
        </Group>
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
      <Table.ScrollContainer minWidth={900} type="native">
        <Table striped withRowBorders={false} verticalSpacing="md">
          <Table.Thead>
            <Table.Tr>
              <Table.Th>
                <Checkbox
                  aria-label="Select all members on this page"
                  checked={pageSelected}
                  indeterminate={selectedOnPage > 0 && !pageSelected}
                  disabled={visibleMembers.length === 0}
                  onChange={(event) =>
                    select(visibleMembers, event.currentTarget.checked)
                  }
                />
              </Table.Th>
              <Table.Th>ID</Table.Th>
              <Table.Th>Email</Table.Th>
              <Table.Th>First name</Table.Th>
              <Table.Th>Last name</Table.Th>
              <Table.Th>Discord</Table.Th>
              <Table.Th>Role</Table.Th>
              <Table.Th>Additional info</Table.Th>
              <Table.Th>Actions</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody aria-busy={members.isPending}>
            {members.isPending && (
              <MembersTableSkeleton rows={membersPerPage} />
            )}
            {visibleMembers.map((member) => (
              <MemberTableRow
                key={member.member_id}
                member={member}
                expanded={expandedId === member.member_id}
                selected={selectedIds.has(member.member_id)}
                onSelect={(selected) => select([member], selected)}
                onToggle={() =>
                  setExpandedId((current) =>
                    current === member.member_id ? null : member.member_id,
                  )
                }
                onEdit={setEditing}
                onArchive={(target) => setArchivingIds([target.member_id])}
              />
            ))}
            {!members.isPending && matchingCount === 0 && (
              <Table.Tr>
                <Table.Td colSpan={9}>
                  <Text fw={500} ta="center">
                    {members.isError ?
                      "Member list unavailable"
                    : hasSearch ?
                      "No members match your search"
                    : roles.length > 0 ?
                      "No members match these filters"
                    : "Members will appear here"}
                  </Text>
                </Table.Td>
              </Table.Tr>
            )}
          </Table.Tbody>
        </Table>
      </Table.ScrollContainer>
      <Group justify="center" mt="md">
        <Skeleton
          visible={members.isPending}
          height={members.isPending ? 32 : undefined}
          width={members.isPending ? 180 : "auto"}
        >
          <Pagination
            total={pageCount}
            value={currentPage}
            onChange={(value) => {
              setPage(value);
              setExpandedId(null);
            }}
            disabled={members.isPending || pageCount === 1}
          />
        </Skeleton>
      </Group>
    </>
  );
}
