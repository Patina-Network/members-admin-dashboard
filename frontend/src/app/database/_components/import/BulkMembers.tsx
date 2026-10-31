import {
  Alert,
  Badge,
  Button,
  Code,
  Group,
  Stack,
  Table,
  Text,
  ThemeIcon,
} from "@mantine/core";
import { Dropzone } from "@mantine/dropzone";
import { IconFileSpreadsheet, IconUpload, IconX } from "@tabler/icons-react";
import { useRef, useState } from "react";

import type {
  BulkMembersProps,
  CsvMemberRow,
} from "@/app/database/_components/shared/types";

import {
  columns,
  maxCsvBytes,
  maxMembers,
  parseMembersCsv,
} from "@/app/database/_components/import/csv";
import { useBulkAddMembersMutation } from "@/lib/api/queries/members";

export default function BulkMembers({ onBack, onPending }: BulkMembersProps) {
  const [rows, setRows] = useState<CsvMemberRow[]>([]);
  const [fileName, setFileName] = useState("");
  const [error, setError] = useState("");
  const [reading, setReading] = useState(false);
  const mutation = useBulkAddMembersMutation();
  const busy = useRef(false);

  const reset = () => {
    setRows([]);
    setFileName("");
    setError("");
    mutation.reset();
  };

  const upload = async (files: File[]) => {
    if (busy.current || !files[0]) return;
    busy.current = true;
    setReading(true);
    onPending(true);
    setError("");

    try {
      setRows(parseMembersCsv(await files[0].text()));
      setFileName(files[0].name);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Unable to read CSV.");
    } finally {
      busy.current = false;
      setReading(false);
      onPending(false);
    }
  };

  const submit = () => {
    if (busy.current) return;
    busy.current = true;
    onPending(true);

    mutation.mutate(
      rows.map((row) => row.member),
      {
        onSettled: () => {
          busy.current = false;
          onPending(false);
        },
      },
    );
  };

  const previewColumns = columns.filter(
    (column, index) =>
      index < 3 || rows.some(({ member }) => member[column] != null),
  );
  const results = new Map(
    mutation.data?.results.map((row) => [row.index, row]),
  );
  const addedCount =
    mutation.data?.results.filter((row) => row.member).length ?? 0;

  return (
    <Stack gap="md">
      {(error || mutation.error) && (
        <Alert
          color="red"
          title={error ? "Check your CSV" : "Import could not be confirmed"}
          role="alert"
        >
          {error || mutation.error?.message}
        </Alert>
      )}
      {!rows.length ?
        <>
          <Text size="sm" c="dimmed">
            Choose a CSV to preview before adding members.
          </Text>
          <Dropzone
            onDrop={(files) => {
              void upload(files);
            }}
            onReject={() =>
              setError(
                "Choose one .csv file of 1 MB or less. Split larger files into smaller imports.",
              )
            }
            accept={{ "text/csv": [".csv"] }}
            maxSize={maxCsvBytes}
            maxFiles={1}
            loading={reading}
            inputProps={{ "aria-label": "Choose CSV file" }}
            radius="md"
            p="lg"
          >
            <Group
              justify="center"
              gap="md"
              wrap="nowrap"
              py="sm"
              style={{ pointerEvents: "none" }}
            >
              <ThemeIcon size={40} radius="md" variant="light" color="gray">
                <Dropzone.Accept>
                  <IconUpload size={22} />
                </Dropzone.Accept>
                <Dropzone.Reject>
                  <IconX size={22} />
                </Dropzone.Reject>
                <Dropzone.Idle>
                  <IconFileSpreadsheet size={22} />
                </Dropzone.Idle>
              </ThemeIcon>
              <Stack gap={4}>
                <Text size="sm" fw={500}>
                  Drag a CSV here or click to select one
                </Text>
                <Text size="xs" c="dimmed">
                  Up to 1 MB · {maxMembers.toLocaleString()} members per file
                </Text>
              </Stack>
            </Group>
          </Dropzone>
          <Stack gap="md">
            <Stack gap={6}>
              <Text size="sm" fw={500}>
                Required columns
              </Text>
              <Code
                style={{ alignSelf: "flex-start", overflowWrap: "anywhere" }}
              >
                {columns.slice(0, 3).join(",")}
              </Code>
              <Text size="xs" c="dimmed">
                Include these exact headers and one member per row.
              </Text>
            </Stack>
            <Stack gap={6}>
              <Text size="sm" fw={500}>
                Optional columns — leave blank if unused
              </Text>
              <Group gap={6}>
                {columns.slice(3).map((column, index) => (
                  <Text key={column} component="code" size="xs" c="dimmed">
                    {column}
                    {index < columns.length - 4 ? "," : ""}
                  </Text>
                ))}
              </Group>
            </Stack>
          </Stack>
          <Text size="xs" c="dimmed">
            Duplicate emails or social accounts fail. Existing members stay
            unchanged.
          </Text>
          <Group justify="flex-end">
            <Button variant="default" onClick={onBack} disabled={reading}>
              Back
            </Button>
          </Group>
        </>
      : <>
          <Stack gap="xs">
            <Group justify="space-between" align="flex-start">
              <Text size="sm" c="dimmed" style={{ overflowWrap: "anywhere" }}>
                File name: {fileName}
              </Text>
              {mutation.data && (
                <Group gap="xs">
                  <Badge color="green">{addedCount} added</Badge>
                  <Badge color={addedCount === rows.length ? "gray" : "red"}>
                    {rows.length - addedCount} failed
                  </Badge>
                </Group>
              )}
            </Group>
            <Text size="sm" role="status" aria-live="polite">
              {mutation.data ?
                `${addedCount} added, ${rows.length - addedCount} failed`
              : mutation.isPending ?
                "Adding members… Please keep this window open."
              : mutation.error ?
                "Results are unconfirmed. Refresh the member table before retrying."
              : `Ready to add ${rows.length} members. Click "Add All" to confirm.`
              }
            </Text>
            {!mutation.data && !mutation.error && (
              <Text size="xs" c="dimmed">
                Please review all fields, scroll horizontally for optional
                columns.
              </Text>
            )}
          </Stack>
          <Table.ScrollContainer
            minWidth={Math.max(650, previewColumns.length * 150 + 180)}
            mah={360}
            type="native"
          >
            <Table striped highlightOnHover withTableBorder>
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>CSV row</Table.Th>
                  <Table.Th>Result</Table.Th>
                  {previewColumns.map((column) => (
                    <Table.Th key={column}>{column}</Table.Th>
                  ))}
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {rows.map(({ member, sourceRow }, index) => {
                  const result = results.get(index);
                  return (
                    <Table.Tr key={sourceRow}>
                      <Table.Td>{sourceRow}</Table.Td>
                      <Table.Td>
                        <Text
                          size="sm"
                          c={
                            result?.member ? "green"
                            : result ?
                              "red"
                            : "dimmed"
                          }
                        >
                          {result?.member ?
                            "Added"
                          : (result?.error ??
                            (mutation.error ? "Unconfirmed"
                            : mutation.isPending ? "Adding…"
                            : "Ready"))
                          }
                        </Text>
                      </Table.Td>
                      {previewColumns.map((column) => (
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
            <Button
              variant="default"
              onClick={reset}
              disabled={mutation.isPending}
            >
              {mutation.data || mutation.error ? "Back to upload" : "Cancel"}
            </Button>
            {!mutation.data && !mutation.error && (
              <Button
                onClick={submit}
                loading={mutation.isPending}
                disabled={mutation.isPending}
              >
                Add All
              </Button>
            )}
          </Group>
        </>
      }
    </Stack>
  );
}
