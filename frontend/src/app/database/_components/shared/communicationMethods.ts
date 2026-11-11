import type { CommunicationMethod } from "@/lib/api/generated/types.gen";

export const communicationLabels: Record<CommunicationMethod, string> = {
  email: "Email",
  phone_number: "Phone Number",
  discord: "Discord",
  instagram: "Instagram",
  linked_in: "LinkedIn",
};

export const communicationOptions = Object.entries(communicationLabels).map(
  ([value, label]) => ({ value, label }),
);
