import {
  Box,
  Button,
  Group,
  Pagination,
  Table,
  Text,
  Title,
} from "@mantine/core";

export default function DatabasePage() {
  return (
    <>
      <Group justify="space-between" mb="md">
        <Box>
          <Title order={2}>Database</Title>
          <Text c="dimmed">— members</Text>
        </Box>
        <Button type="button">Add</Button>
      </Group>
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
            <Table.Tr>
              <Table.Td colSpan={7}>
                <Text fw={500} ta="center">
                  Members will appear here
                </Text>
              </Table.Td>
            </Table.Tr>
          </Table.Tbody>
        </Table>
      </Box>
      <Group justify="center" mt="md">
        <Pagination total={1} value={1} disabled />
      </Group>
    </>
  );
}
