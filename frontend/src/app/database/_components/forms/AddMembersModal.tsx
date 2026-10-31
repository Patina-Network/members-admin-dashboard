import { Alert, Button, Group, Modal, Stack } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useRef, useState } from "react";

import type { AddMembersModalProps } from "@/app/database/_components/shared/types";
import type { NewMember } from "@/lib/api/generated/types.gen";

import MemberFields from "@/app/database/_components/forms/MemberFields";
import BulkMembers from "@/app/database/_components/import/BulkMembers";
import { useMemberForm } from "@/app/database/_components/shared/hook";
import { useAddMemberMutation } from "@/lib/api/queries/members";

export default function AddMembersModal({
  opened,
  onCreated,
  onClose,
}: AddMembersModalProps) {
  const [bulkPending, setBulkPending] = useState(false);
  const [bulk, setBulk] = useState(false);
  const [details, setDetails] = useState<string | null>(null);
  const addMember = useAddMemberMutation();
  const form = useMemberForm();
  const submittingRef = useRef(false);

  const close = () => {
    if (submittingRef.current || bulkPending) return;
    setBulk(false);
    onClose();
    setDetails(null);
    form.reset();
    addMember.reset();
  };

  const onSubmit = (values: NewMember) => {
    if (submittingRef.current || bulkPending) return;
    submittingRef.current = true;

    addMember.mutate(values, {
      onSuccess: (member) => {
        onCreated(member);
        notifications.show({ message: "Member added.", color: "green" });
        form.reset();
        addMember.reset();
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
      title={bulk ? "Import members from CSV" : "Add member"}
      size="lg"
      centered
      closeOnClickOutside={!(addMember.isPending || bulkPending)}
      closeOnEscape={!(addMember.isPending || bulkPending)}
      withCloseButton={!(addMember.isPending || bulkPending)}
    >
      {bulk ?
        <BulkMembers onBack={() => setBulk(false)} onPending={setBulkPending} />
      : <>
          <Button
            variant="light"
            mb="md"
            onClick={() => setBulk(true)}
            disabled={addMember.isPending}
          >
            Upload Bulk Member(s)
          </Button>
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
              <MemberFields
                form={form}
                pending={addMember.isPending}
                details={details}
                setDetails={setDetails}
              />
              <Group justify="flex-end">
                <Button
                  variant="default"
                  disabled={addMember.isPending}
                  onClick={close}
                >
                  Cancel
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
        </>
      }
    </Modal>
  );
}
