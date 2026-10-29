import { Alert, Button, Group, Modal, Stack, Text } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useRef, useState } from "react";

import type {
  EditMemberFormProps,
  EditMemberModalProps,
} from "@/app/database/_components/shared/types";
import type { NewMember } from "@/lib/api/generated/types.gen";

import MemberFields from "@/app/database/_components/forms/MemberFields";
import { useEditMemberForm } from "@/app/database/_components/shared/hook";
import {
  useMemberDetailsQuery,
  useUpdateMemberMutation,
} from "@/lib/api/queries/members";

export default function EditMemberModal({
  member,
  onClose,
}: EditMemberModalProps) {
  const query = useMemberDetailsQuery(member.member_id);
  return query.isSuccess ?
      <EditMemberForm member={query.data} onClose={onClose} />
    : <Modal opened onClose={onClose} title="Edit member" size="lg" centered>
        {query.isError ?
          <Alert color="red" role="alert">
            {query.error.message}
            <Button onClick={() => void query.refetch()}>Retry</Button>
          </Alert>
        : <Text>Loading member details…</Text>}
      </Modal>;
}

function EditMemberForm({ member, onClose }: EditMemberFormProps) {
  const [details, setDetails] = useState<string | null>(null);
  const { form, changedFields } = useEditMemberForm(member);
  const update = useUpdateMemberMutation();
  const submitting = useRef(false);
  const submit = (values: NewMember) => {
    if (submitting.current) return;
    submitting.current = true;
    update.mutate(
      {
        memberId: member.member_id,
        body: changedFields(values),
      },
      {
        onSuccess: () => {
          notifications.show({ message: "Member updated.", color: "green" });
          onClose();
        },
        onSettled: () => {
          submitting.current = false;
        },
      },
    );
  };
  return (
    <Modal
      opened
      onClose={() => {
        if (!submitting.current) onClose();
      }}
      title="Edit member"
      size="lg"
      centered
      closeOnClickOutside={!update.isPending}
      closeOnEscape={!update.isPending}
      withCloseButton={!update.isPending}
    >
      <form
        noValidate
        onSubmit={form.onSubmit(submit, (errors) => {
          if (
            [
              "discordId",
              "discordUsername",
              "linkedinUsername",
              "instagramUsername",
            ].some((field) => errors[field])
          )
            setDetails("additional-details");
        })}
      >
        <Stack>
          {update.error && (
            <Alert color="red" role="alert">
              {update.error.message}
            </Alert>
          )}
          <Text size="sm" c="dimmed">
            Member #{member.member_id}
          </Text>
          <MemberFields
            joinedAtReadOnly
            form={form}
            pending={update.isPending}
            details={details}
            setDetails={setDetails}
          />
          <Group justify="flex-end">
            <Button
              variant="default"
              disabled={update.isPending}
              onClick={() => {
                if (!submitting.current) onClose();
              }}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              loading={update.isPending}
              disabled={update.isPending}
            >
              Save changes
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}
