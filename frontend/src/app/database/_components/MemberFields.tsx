import {
  Accordion,
  NativeSelect,
  SimpleGrid,
  TextInput,
  Textarea,
} from "@mantine/core";

import type { MemberFieldsProps } from "@/app/database/_components/types";
import type { NewMember } from "@/lib/api/generated/types.gen";

const roleLabels: Record<NonNullable<NewMember["role"]>, string> = {
  board: "Board",
  mentor: "Mentor",
  mentee: "Mentee",
};
const roleOptions = [
  { value: "", label: "Select a role" },
  ...Object.entries(roleLabels).map(([value, label]) => ({ value, label })),
];

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
              <TextInput
                {...form.getInputProps("career")}
                label="Career"
                disabled={pending}
              />
              <TextInput
                {...form.getInputProps("industry")}
                label="Industry"
                disabled={pending}
              />
            </SimpleGrid>
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
