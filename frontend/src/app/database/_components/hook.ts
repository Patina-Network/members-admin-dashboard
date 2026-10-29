import { useForm } from "@mantine/form";

import type { MemberFormValues } from "@/app/database/_components/types";
import type { NewMember } from "@/lib/api/generated/types.gen";

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

export const useMemberForm = () => {
  return useForm<MemberFormValues, (values: MemberFormValues) => NewMember>({
    initialValues,
    validate: {
      discordId: (value, values) => {
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
        values.discordId.trim() && !value.trim() ?
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
};
