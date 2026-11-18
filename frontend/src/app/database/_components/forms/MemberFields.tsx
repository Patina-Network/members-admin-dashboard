import {
  Accordion,
  ActionIcon,
  Button,
  Checkbox,
  Group,
  Input,
  NativeSelect,
  SimpleGrid,
  Stack,
  TextInput,
  Textarea,
} from "@mantine/core";
import { IconX } from "@tabler/icons-react";

import type { MemberFieldsProps } from "@/app/database/_components/shared/types";

import SuggestionField from "@/app/database/_components/forms/SuggestionField";
import { communicationOptions } from "@/app/database/_components/shared/communicationMethods";
import { roleOptions } from "@/app/database/_components/shared/memberRoles";

export default function MemberFields({
  form,
  joinedAtReadOnly = false,
  pending,
  details,
  setDetails,
}: MemberFieldsProps) {
  return (
    <>
      <TextInput
        {...form.getInputProps("email")}
        label="Email"
        type="email"
        required
        disabled={pending}
      />
      <TextInput
        {...form.getInputProps("firstName")}
        label="First Name"
        required
        disabled={pending}
      />
      <TextInput
        {...form.getInputProps("lastName")}
        label="Last Name"
        required
        disabled={pending}
      />
      <TextInput
        {...form.getInputProps("joinedAt")}
        label="Joined At"
        type="datetime-local"
        readOnly={joinedAtReadOnly}
        disabled={pending}
      />
      <NativeSelect
        {...form.getInputProps("role")}
        label="Role"
        data={roleOptions}
        disabled={pending}
      />
      <Accordion
        variant="contained"
        radius="md"
        value={details}
        onChange={setDetails}
      >
        <Accordion.Item value="additional-details">
          <Accordion.Control>
            Additional Details: (Social & Professional)
          </Accordion.Control>
          <Accordion.Panel>
            <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
              <TextInput
                {...form.getInputProps("discordId")}
                label="Discord ID"
                disabled={pending}
              />
              <TextInput
                {...form.getInputProps("discordUsername")}
                label="Discord Username"
                disabled={pending}
              />
              <TextInput
                {...form.getInputProps("linkedinUsername")}
                label="LinkedIn Username"
                disabled={pending}
              />
              <TextInput
                {...form.getInputProps("instagramUsername")}
                label="Instagram Username"
                disabled={pending}
              />
              <SuggestionField
                form={form}
                field="career"
                kind="careers"
                label="Career"
                disabled={pending}
              />
              <SuggestionField
                form={form}
                field="industry"
                kind="industries"
                label="Industry"
                disabled={pending}
              />
              <TextInput
                {...form.getInputProps("school")}
                label="School"
                disabled={pending}
              />
              <TextInput
                {...form.getInputProps("gradDate")}
                label="Grad Date"
                placeholder="e.g. Spring 2028"
                disabled={pending}
              />
              <Stack gap="xs" style={{ gridColumn: "1 / -1" }}>
                <Input.Label>Secondary Emails</Input.Label>
                {form.values.secondaryEmails.map((_, index) => (
                  <Group key={index} gap="xs" wrap="nowrap">
                    <TextInput
                      {...form.getInputProps(`secondaryEmails.${index}`, {
                        withError: false,
                      })}
                      aria-label={`Secondary email ${index + 1}`}
                      type="email"
                      disabled={pending}
                      style={{ flex: 1 }}
                    />
                    <ActionIcon
                      variant="subtle"
                      color="red"
                      aria-label={`Remove secondary email ${index + 1}`}
                      disabled={pending}
                      onClick={() =>
                        form.removeListItem("secondaryEmails", index)
                      }
                    >
                      <IconX size={16} />
                    </ActionIcon>
                  </Group>
                ))}
                {form.errors.secondaryEmails && (
                  <Input.Error>{form.errors.secondaryEmails}</Input.Error>
                )}
                <Button
                  variant="subtle"
                  size="xs"
                  w="fit-content"
                  disabled={pending}
                  onClick={() => form.insertListItem("secondaryEmails", "")}
                >
                  Add secondary email
                </Button>
              </Stack>
            </SimpleGrid>
            <Checkbox.Group
              {...form.getInputProps("preferredCommunications")}
              label="Preferred Communications"
              mt="sm"
            >
              <Group mt="xs">
                {communicationOptions.map(({ value, label }) => (
                  <Checkbox
                    key={value}
                    value={value}
                    label={label}
                    disabled={pending}
                  />
                ))}
              </Group>
            </Checkbox.Group>
          </Accordion.Panel>
        </Accordion.Item>
      </Accordion>
      <Textarea
        {...form.getInputProps("profile")}
        label="Profile notes"
        autosize
        minRows={3}
        maxRows={6}
        disabled={pending}
      />
    </>
  );
}
