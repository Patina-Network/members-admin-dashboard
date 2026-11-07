import {
  Alert,
  Badge,
  Button,
  Checkbox,
  Group,
  Stack,
  Table,
  Text,
} from "@mantine/core";

import type { BulkMembersProps } from "@/app/database/_components/shared/types";

import CsvUpload from "@/app/database/_components/import/CsvUpload";
import { useBulkImport } from "@/app/database/_components/shared/hook";

export default function BulkMembers({ onBack, onPending }: BulkMembersProps) {
  const bulk = useBulkImport(onPending);
  const pending = bulk.phase === "pending";
  const settled = bulk.phase === "done" || bulk.phase === "unconfirmed";

  return (
    <Stack gap="md">
      {bulk.alert && (
        <Alert color="red" title={bulk.alert.title} role="alert">
          {bulk.alert.message}
        </Alert>
      )}
      {!bulk.rows.length ?
        <CsvUpload
          reading={bulk.reading}
          onDrop={bulk.upload}
          onReject={bulk.rejectFile}
          onBack={onBack}
        />
      : <>
          <Stack gap="xs">
            <Group justify="space-between" align="flex-start">
              <Text size="sm" c="dimmed" style={{ overflowWrap: "anywhere" }}>
                File name: {bulk.fileName}
              </Text>
              {bulk.phase === "done" && (
                <Group gap="xs">
                  {bulk.tallies.map(({ label, count, color }) => (
                    <Badge key={label} color={color}>
                      {count} {label}
                    </Badge>
                  ))}
                </Group>
              )}
            </Group>
            <Text size="sm" role="status" aria-live="polite">
              {bulk.status}
            </Text>
            {!settled && (
              <>
                <Checkbox
                  label="Update existing members"
                  description="Rows whose email already exists update that member: names and filled-in cells overwrite stored values, blank cells keep them."
                  checked={bulk.upsert}
                  onChange={(event) =>
                    bulk.setUpsert(event.currentTarget.checked)
                  }
                  disabled={pending}
                />
                <Text size="xs" c="dimmed">
                  Please review all fields, scroll horizontally for optional
                  columns.
                </Text>
              </>
            )}
          </Stack>
          <Table.ScrollContainer
            minWidth={Math.max(650, bulk.previewColumns.length * 150 + 180)}
            mah={360}
            type="native"
          >
            <Table striped highlightOnHover withTableBorder>
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>CSV row</Table.Th>
                  <Table.Th>Result</Table.Th>
                  {bulk.previewColumns.map((column) => (
                    <Table.Th key={column}>{column}</Table.Th>
                  ))}
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {bulk.rows.map(({ member, sourceRow }, index) => {
                  const { label, color } = bulk.rowStatus(index);
                  return (
                    <Table.Tr key={sourceRow}>
                      <Table.Td>{sourceRow}</Table.Td>
                      <Table.Td>
                        <Text size="sm" c={color}>
                          {label}
                        </Text>
                      </Table.Td>
                      {bulk.previewColumns.map((column) => (
                        <Table.Td key={column}>
                          <Text
                            size="sm"
                            maw={240}
                            lineClamp={2}
                            title={member[column] ?? undefined}
                          >
                            {member[column] || "—"}
                          </Text>
                        </Table.Td>
                      ))}
                    </Table.Tr>
                  );
                })}
              </Table.Tbody>
            </Table>
          </Table.ScrollContainer>
          <Group justify="flex-end">
            <Button variant="default" onClick={bulk.reset} disabled={pending}>
              {settled ? "Back to upload" : "Cancel"}
            </Button>
            {!settled && (
              <Button
                onClick={bulk.submit}
                loading={pending}
                disabled={pending}
              >
                Import
              </Button>
            )}
          </Group>
        </>
      }
    </Stack>
  );
}
