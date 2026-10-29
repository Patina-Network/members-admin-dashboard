import { useForm } from "@mantine/form";

import type { MemberFormValues } from "@/app/database/_components/types";
import type {
  MemberDetails,
  MemberUpdate,
  NewMember,
} from "@/lib/api/generated/types.gen";

function validateSocialUsername(value: string) {
  return /[\s/@?#]/u.test(value.trim()) ?
      "Please enter a username without a URL, @, or spaces"
    : null;
}

const initialValues: MemberFormValues = {
  email: "",
  firstName: "",
  lastName: "",
  joinedAt: "",
  discordId: "",
  discordUsername: "",
  linkedinUsername: "",
  instagramUsername: "",
  career: "",
  industry: "",
  profile: "",
  role: "",
};

export const useMemberForm = (
  initial?: Partial<MemberFormValues>,
  editing = false,
) => {
  const discordChanged = (values: MemberFormValues) =>
    !editing ||
    values.discordId !== initial?.discordId ||
    values.discordUsername !== initial?.discordUsername;
  const form = useForm<
    MemberFormValues,
    (values: MemberFormValues) => NewMember
  >({
    initialValues: { ...initialValues, ...initial },
    onValuesChange: (current, previous) => {
      if (
        current.discordId !== previous.discordId ||
        current.discordUsername !== previous.discordUsername
      ) {
        form.clearFieldError("discordId");
        form.clearFieldError("discordUsername");
      }
    },
    validate: {
      discordId: (value, values) => {
        if (!discordChanged(values)) return null;
        const id = value.trim();
        if (!id)
          return values.discordUsername.trim() ?
              "Discord ID is required with a username"
            : null;
        return (
            /^\d+$/.test(id) &&
              BigInt(id) > 0n &&
              BigInt(id) <= 18446744073709551615n
          ) ?
            null
          : "Enter a valid Discord ID";
      },
      discordUsername: (value, values) =>
        discordChanged(values) && values.discordId.trim() && !value.trim() ?
          "Discord username is required with an ID"
        : null,
      linkedinUsername: validateSocialUsername,
      instagramUsername: validateSocialUsername,
      email: (value) =>
        /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.trim()) ? null : (
          "Enter a valid email"
        ),
      firstName: (value) => (value.trim() ? null : "First name is required"),
      lastName: (value) => (value.trim() ? null : "Last name is required"),
      joinedAt: (value) => {
        if (editing) return null;
        if (!value) return null;
        const date = new Date(value);
        if (Number.isNaN(date.getTime())) return "Enter a valid date and time";
        return date.getTime() > Date.now() ?
            "Joined at must not be in the future"
          : null;
      },
    },
    transformValues: (values): NewMember => ({
      email: values.email.trim(),
      first_name: values.firstName.trim(),
      last_name: values.lastName.trim(),
      joined_at:
        values.joinedAt ? new Date(values.joinedAt).toISOString() : null,
      discord_id: values.discordId.trim() || null,
      discord_username: values.discordUsername.trim() || null,
      linkedin_username: values.linkedinUsername.trim() || null,
      instagram_username: values.instagramUsername.trim() || null,
      career: values.career.trim() || null,
      industry: values.industry.trim() || null,
      profile: values.profile.trim() || null,
      role: values.role || null,
    }),
  });
  return form;
};

export const useEditMemberForm = (member: MemberDetails) => {
  const date = member.joined_at ? new Date(member.joined_at) : null;
  const joinedAt =
    date ?
      new Date(date.getTime() - date.getTimezoneOffset() * 60000)
        .toISOString()
        .slice(0, 19)
    : "";
  const form = useMemberForm(
    {
      email: member.email,
      firstName: member.first_name,
      lastName: member.last_name,
      joinedAt,
      linkedinUsername: member.linkedin_username ?? "",
      instagramUsername: member.instagram_username ?? "",
      career: member.career ?? "",
      industry: member.industry ?? "",
      profile: member.profile ?? "",
      role: member.role ?? "",
      discordId: member.discord_id ?? "",
      discordUsername: member.discord_username ?? "",
    },
    true,
  );
  const changedFields = (values: NewMember): MemberUpdate => {
    const body: MemberUpdate = { expected_updated_at: member.updated_at };
    if (form.isDirty("email")) body.email = values.email;
    if (form.isDirty("firstName")) body.first_name = values.first_name;
    if (form.isDirty("lastName")) body.last_name = values.last_name;
    if (form.isDirty("discordId") || form.isDirty("discordUsername")) {
      body.discord_id = values.discord_id;
      body.discord_username = values.discord_username;
    }
    if (form.isDirty("linkedinUsername"))
      body.linkedin_username = values.linkedin_username;
    if (form.isDirty("instagramUsername"))
      body.instagram_username = values.instagram_username;
    if (form.isDirty("career")) body.career = values.career;
    if (form.isDirty("industry")) body.industry = values.industry;
    if (form.isDirty("profile")) body.profile = values.profile;
    if (form.isDirty("role")) body.role = values.role;
    return body;
  };
  return { form, changedFields };
};
