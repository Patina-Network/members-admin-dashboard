import type { CsvMemberRow } from "@/app/database/_components/shared/types";
import type { NewMember } from "@/lib/api/generated/types.gen";

import { isMemberRole } from "@/app/database/_components/shared/memberRoles";
import {
  validateDiscordId,
  validateEmail,
  validateSocialUsername,
} from "@/app/database/_components/shared/memberValidation";

export const maxCsvBytes = 1024 * 1024;
export const maxBulkRequestBytes = 2 * 1024 * 1024;
export const maxMembers = 1000;

export const columns = [
  "email",
  "first_name",
  "last_name",
  "joined_at",
  "discord_id",
  "discord_username",
  "linkedin_username",
  "instagram_username",
  "career",
  "industry",
  "profile",
  "role",
] as const;

export function parseMembersCsv(text: string): CsvMemberRow[] {
  if (new TextEncoder().encode(text).length > maxCsvBytes)
    throw new Error(
      "Choose a CSV file of 1 MB or less. Split larger files into smaller imports.",
    );
  const records: { sourceRow: number; values: string[] }[] = [];
  let sourceRow = 1;
  let row: string[] = [],
    field = "",
    quoted = false,
    closed = false;
  text = text.replace(/^\uFEFF/, "");
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (quoted) {
      if (char === '"' && text[i + 1] === '"') {
        field += '"';
        i++;
      } else if (char === '"') {
        quoted = false;
        closed = true;
      } else field += char;
    } else if (char === '"' && !field && !closed) quoted = true;
    else if (char === "," || char === "\n" || char === "\r") {
      row.push(field.trim());
      field = "";
      closed = false;
      if (char !== ",") {
        if (row.some(Boolean)) records.push({ sourceRow, values: row });
        row = [];
        sourceRow++;
        if (char === "\r" && text[i + 1] === "\n") i++;
      }
    } else {
      if (closed || char === '"') throw new Error("Malformed CSV quoting.");
      field += char;
    }
  }
  if (quoted) throw new Error("Unclosed quoted CSV field.");
  row.push(field.trim());
  if (row.some(Boolean)) records.push({ sourceRow, values: row });
  const header = records.shift()?.values;
  if (
    !header ||
    new Set(header).size !== header.length ||
    header.some((key) => !columns.includes(key as (typeof columns)[number])) ||
    !columns.slice(0, 3).every((key) => header.includes(key))
  )
    throw new Error(
      "Include email, first_name, last_name headers and only supported columns.",
    );
  if (!records.length || records.length > maxMembers)
    throw new Error("Upload between 1 and 1,000 members.");
  const rows = records.map(({ values, sourceRow }) => {
    const fail = (message: string): never => {
      throw new Error(`Row ${sourceRow}: ${message}`);
    };
    if (values.length !== header.length)
      fail("Column count does not match headers.");
    const member = Object.fromEntries(
      header.map((key, i) => [key, values[i] || null]),
    ) as NewMember;
    if (!member.first_name || !member.last_name)
      fail("First and last names are required.");
    const emailError = validateEmail(member.email ?? "");
    if (emailError) fail(emailError);
    if (member.joined_at) {
      if (!isValidJoinedAt(member.joined_at))
        fail(
          "joined_at must be a valid past ISO date/time with timezone (for example, 2026-01-01T12:00:00Z).",
        );
      member.joined_at = new Date(member.joined_at).toISOString();
    }
    if (!!member.discord_id !== !!member.discord_username)
      fail("Discord ID and username must be provided together.");
    const discordError = validateDiscordId(
      member.discord_id ?? "",
      member.discord_username ?? "",
    );
    if (discordError) fail(discordError);
    for (const key of ["linkedin_username", "instagram_username"] as const) {
      const error = validateSocialUsername(member[key] ?? "");
      if (error) fail(`${key}: ${error}`);
    }
    if (member.role && !isMemberRole(member.role))
      fail("role must be board, mentor, or mentee.");
    return { sourceRow, member };
  });
  if (
    new TextEncoder().encode(
      JSON.stringify({ members: rows.map((row) => row.member) }),
    ).length > maxBulkRequestBytes
  )
    throw new Error(
      "This CSV expands beyond the import size limit. Split it into smaller files or shorten long text fields.",
    );
  return rows;
}

function isValidJoinedAt(value: string) {
  const match =
    /^(\d{4})-(\d{2})-(\d{2})T([01]\d|2[0-3]):([0-5]\d):([0-5]\d)(?:\.\d{1,9})?(?:Z|[+-](?:[01]\d|2[0-3]):[0-5]\d)$/.exec(
      value,
    );
  if (!match) return false;
  const [, year, month, day] = match;
  const calendarDate = new Date(`${year}-${month}-${day}T00:00:00Z`);
  const timestamp = Date.parse(value);
  return (
    calendarDate.getUTCFullYear() === Number(year) &&
    calendarDate.getUTCMonth() + 1 === Number(month) &&
    calendarDate.getUTCDate() === Number(day) &&
    Number.isFinite(timestamp) &&
    timestamp <= Date.now()
  );
}
