import { Skeleton, Table, VisuallyHidden } from "@mantine/core";

export default function MembersTableSkeleton({ rows }: { rows: number }) {
  return (
    <>
      {Array.from({ length: rows }, (_, row) => (
        <Table.Tr key={row}>
          {Array.from({ length: 8 }, (_, column) => (
            <Table.Td key={column}>
              {row === 0 && column === 0 && (
                <VisuallyHidden role="status">Loading members…</VisuallyHidden>
              )}
              <Skeleton height={20} radius="sm" aria-hidden="true" />
            </Table.Td>
          ))}
        </Table.Tr>
      ))}
    </>
  );
}
