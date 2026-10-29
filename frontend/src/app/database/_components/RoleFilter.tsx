import { Button, Checkbox, Popover, Stack } from "@mantine/core";

import type { RoleFilterProps } from "@/app/database/_components/types";

import { roleOptions } from "@/app/database/_components/memberRoles";

export default function RoleFilter({ roles, onChange }: RoleFilterProps) {
  return (
    <Popover position="bottom-end" shadow="md">
      <Popover.Target>
        <Button type="button" variant="default">
          Filters
        </Button>
      </Popover.Target>
      <Popover.Dropdown>
        <Checkbox.Group label="Role" value={roles} onChange={onChange}>
          <Stack gap="xs" mt="xs">
            {roleOptions
              .filter((option) => option.value)
              .map((option) => (
                <Checkbox
                  key={option.value}
                  value={option.value}
                  label={option.label}
                />
              ))}
          </Stack>
        </Checkbox.Group>
      </Popover.Dropdown>
    </Popover>
  );
}
