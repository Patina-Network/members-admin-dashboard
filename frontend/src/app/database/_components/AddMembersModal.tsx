import {
  Accordion,
  Alert,
  Button,
  FileInput,
  Group,
  Modal,
  NativeSelect,
  Paper,
  SimpleGrid,
  Stack,
  Text,
  TextInput,
  Textarea,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconFileSpreadsheet } from "@tabler/icons-react";
import { useRef, useState } from "react";

import type {
  AddMembersModalProps,
  AddMembersView,
} from "@/app/database/_components/types";
import type { NewMember } from "@/lib/api/generated/types.gen";

import { useMemberForm } from "@/app/database/_components/hook";
import { useAddMemberMutation } from "@/lib/api/queries/members";

const roleLabels: Record<NonNullable<NewMember["role"]>, string> = {
  board: "Board",
  mentor: "Mentor",
  mentee: "Mentee",
};
const roleOptions = [
  { value: "", label: "Select a role" },
  ...Object.entries(roleLabels).map(([value, label]) => ({ value, label })),
];

export default function AddMembersModal({
  opened,
  onClose,
}: AddMembersModalProps) {
  const [details, setDetails] = useState<string | null>(null);
  const [view, setView] = useState<AddMembersView>("choose");
  const addMember = useAddMemberMutation();
  const form = useMemberForm();
  const submittingRef = useRef(false);
  const close = () => {
    if (submittingRef.current) return;
    onClose();
    setView("choose");
    setDetails(null);
    form.reset();
    addMember.reset();
  };
  const onSubmit = (values: NewMember) => {
    if (submittingRef.current) return;
    submittingRef.current = true;
    addMember.mutate(values, {
      onSuccess: () => {
        notifications.show({ message: "Member added.", color: "green" });
        form.reset();
        addMember.reset();
        setView("choose");
        setDetails(null);
        onClose();
      },
      onSettled: () => {
        submittingRef.current = false;
      },
    });
  };

  return (
    <Modal
      opened={opened}
      onClose={close}
      title="Add Member(s)"
      size="lg"
      centered
      closeOnClickOutside={!addMember.isPending}
      closeOnEscape={!addMember.isPending}
      withCloseButton={!addMember.isPending}
    >
      {view === "choose" && (
        <Stack>
          <Button onClick={() => setView("one")}>Add One Member</Button>
          <Button variant="light" onClick={() => setView("bulk-upload")}>
            Upload Bulk Member(s)
          </Button>
        </Stack>
      )}
      {view === "one" && (
        <form
          noValidate
          onSubmit={form.onSubmit(onSubmit, (errors) => {
            if (
              [
                "discordId",
                "discordUsername",
                "linkedinUsername",
                "instagramUsername",
                "career",
                "industry",
              ].some((field) => errors[field])
            ) {
              setDetails("additional-details");
            }
          })}
        >
          <Stack>
            {addMember.error && (
              <Alert color="red" role="alert">
                {addMember.error.message}
              </Alert>
            )}
            <TextInput
              {...form.getInputProps("email")}
              label="Email"
              type="email"
              required
              disabled={addMember.isPending}
            />
            <TextInput
              {...form.getInputProps("firstName")}
              label="First Name"
              required
              disabled={addMember.isPending}
            />
            <TextInput
              {...form.getInputProps("lastName")}
              label="Last Name"
              required
              disabled={addMember.isPending}
            />
            <TextInput
              {...form.getInputProps("joinedAt")}
              label="Joined At"
              type="datetime-local"
              disabled={addMember.isPending}
            />
            <NativeSelect
              {...form.getInputProps("role")}
              label="Role"
              data={roleOptions}
              disabled={addMember.isPending}
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
                      disabled={addMember.isPending}
                    />
                    <TextInput
                      {...form.getInputProps("discordUsername")}
                      label="Discord Username"
                      disabled={addMember.isPending}
                    />
                    <TextInput
                      {...form.getInputProps("linkedinUsername")}
                      label="LinkedIn Username"
                      disabled={addMember.isPending}
                    />
                    <TextInput
                      {...form.getInputProps("instagramUsername")}
                      label="Instagram Username"
                      disabled={addMember.isPending}
                    />
                    <TextInput
                      {...form.getInputProps("career")}
                      label="Career"
                      disabled={addMember.isPending}
                    />
                    <TextInput
                      {...form.getInputProps("industry")}
                      label="Industry"
                      disabled={addMember.isPending}
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
              disabled={addMember.isPending}
            />
            <Group justify="flex-end">
              <Button
                variant="default"
                disabled={addMember.isPending}
                onClick={() => {
                  if (submittingRef.current) return;
                  addMember.reset();
                  setView("choose");
                }}
              >
                Back
              </Button>
              <Button
                type="submit"
                loading={addMember.isPending}
                disabled={addMember.isPending}
              >
                Add
              </Button>
            </Group>
          </Stack>
        </form>
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
            <Button
              type="button"
              variant="default"
              onClick={() => setView("choose")}
            >
              Back
            </Button>
          </Group>
        </Stack>
      )}
    </Modal>
  );
}
