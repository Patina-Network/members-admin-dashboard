import { Button, Code, Group, Stack, Text, ThemeIcon } from "@mantine/core";
import { Dropzone } from "@mantine/dropzone";
import { IconFileSpreadsheet, IconUpload, IconX } from "@tabler/icons-react";

import type { CsvUploadProps } from "@/app/database/_components/shared/types";

import {
  maxCsvBytes,
  maxMembers,
  optionalColumns,
  requiredColumns,
} from "@/app/database/_components/import/csv";

export default function CsvUpload({
  reading,
  onDrop,
  onReject,
  onBack,
}: CsvUploadProps) {
  return (
    <>
      <Text size="sm" c="dimmed">
        Choose a CSV to preview before adding members.
      </Text>
      <Dropzone
        onDrop={onDrop}
        onReject={onReject}
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
          <Code style={{ alignSelf: "flex-start", overflowWrap: "anywhere" }}>
            {requiredColumns.join(",")}
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
            {optionalColumns.map((column, index) => (
              <Text key={column} component="code" size="xs" c="dimmed">
                {column}
                {index < optionalColumns.length - 1 && ","}
              </Text>
            ))}
          </Group>
        </Stack>
      </Stack>
      <Text size="xs" c="dimmed">
        Duplicate emails or social accounts fail. Existing members stay
        unchanged unless you choose to update them before importing.
      </Text>
      <Group justify="flex-end">
        <Button variant="default" onClick={onBack} disabled={reading}>
          Back
        </Button>
      </Group>
    </>
  );
}
