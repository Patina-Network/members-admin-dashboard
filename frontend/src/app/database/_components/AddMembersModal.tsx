import {
  Button,
  FileInput,
  Group,
  Modal,
  Paper,
  Stack,
  Table,
  Text,
  TextInput,
} from "@mantine/core";
import { IconFileSpreadsheet } from "@tabler/icons-react";

type View = "choose" | "one" | "bulk-upload" | "bulk-preview";

export default function AddMembersModal({
  opened,
  onClose,
  view = "choose",
}: {
  opened: boolean;
  onClose: () => void;
  view?: View;
}) {
  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title="Add Member(s)"
      size="lg"
      centered
    >
      {view === "choose" && (
        <Stack>
          <Button type="button">Add One Member</Button>
          <Button type="button" variant="light">
            Upload Bulk Member(s)
          </Button>
        </Stack>
      )}
      {view === "one" && (
        <Stack>
          <TextInput label="Email" required />
          <TextInput label="First name" required />
          <TextInput label="Last name" required />
          <TextInput label="Joined at" type="datetime-local" />
          <TextInput label="Discord ID" />
          <TextInput label="Discord username" />
          <Group justify="flex-end">
            <Button type="button" variant="default">
              Back
            </Button>
            <Button type="button">Add</Button>
          </Group>
        </Stack>
      )}
      {view === "bulk-upload" && (
        <Stack>
          <Text size="sm" c="dimmed">
            Upload a CSV with these columns:
            email,first_name,last_name,joined_at,discord_id,discord_username
          </Text>
          <Paper withBorder p="xl">
            <Group justify="center" gap="xl" mih={140}>
              <IconFileSpreadsheet size={40} />
              <Stack gap="xs">
                <Text size="lg">Drag a CSV here or click to select one</Text>
                <Text size="sm" c="dimmed">
                  One member per row. Header row required.
                </Text>
              </Stack>
            </Group>
          </Paper>
          <FileInput disabled placeholder="Or choose a file" />
          <Group justify="flex-end">
            <Button type="button" variant="default">
              Back
            </Button>
          </Group>
        </Stack>
      )}
      {view === "bulk-preview" && (
        <Stack>
          <Text size="sm">Preview members to add</Text>
          <Table striped withTableBorder>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Email</Table.Th>
                <Table.Th>First name</Table.Th>
                <Table.Th>Last name</Table.Th>
                <Table.Th>Joined at</Table.Th>
                <Table.Th>Discord ID</Table.Th>
                <Table.Th>Discord username</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody />
          </Table>
          <Group justify="flex-end">
            <Button type="button" variant="default">
              Cancel
            </Button>
            <Button type="button">Add All</Button>
          </Group>
        </Stack>
      )}
    </Modal>
  );
}
