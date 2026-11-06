import { describe, expect, it } from "vitest";

import {
  maxBulkRequestBytes,
  maxCsvBytes,
  parseMembersCsv,
} from "@/app/database/_components/import/csv";

describe("CSV import", () => {
  it("handles BOM, CRLF, quoted commas, escaped quotes and multiline fields", () => {
    const rows = parseMembersCsv(
      '\uFEFFemail,first_name,last_name,profile\r\na@b.com,"A, B",C,"Hello ""friend""\nworld"\r\n',
    );

    expect(rows[0]?.member).toMatchObject({
      email: "a@b.com",
      first_name: "A, B",
      profile: 'Hello "friend"\nworld',
    });
  });

  it.each([
    "email,first_name,last_name\na@b.com,A",
    'email,first_name,last_name\na@b.com,"A,B',
    "email,first_name,last_name\ninvalid,A,B",
    "email,first_name,last_name,discord_id\na@b.com,A,B,123",
    "email,first_name,last_name,role\na@b.com,A,B,admin",
    "email,email,first_name,last_name\na@b.com,a@b.com,A,B",
  ])("rejects invalid CSV: %s", (csv) => {
    expect(() => parseMembersCsv(csv)).toThrow();
  });

  it("preserves duplicate rows for create operations to report failures", () => {
    expect(
      parseMembersCsv("email,first_name,last_name\na@b.com,A,B\na@b.com,A,B"),
    ).toHaveLength(2);
  });
});

describe("CSV regression cases", () => {
  it.each([
    "2026-02-30T12:00:00Z",
    "2025-02-29T12:00:00Z",
    "2024-04-31T12:00:00Z",
    "2024-13-01T12:00:00Z",
    "2024-01-01T24:00:00Z",
    "2024-01-01T12:00:00",
    "2999-01-01T12:00:00Z",
  ])("rejects invalid or future joined dates: %s", (date) => {
    expect(() =>
      parseMembersCsv(
        `email,first_name,last_name,joined_at\na@b.com,A,B,${date}`,
      ),
    ).toThrow("Row 2: joined_at");
  });

  it("accepts leap days and normalizes timezone offsets", () => {
    const rows = parseMembersCsv(
      "email,first_name,last_name,joined_at\na@b.com,A,B,2024-02-29T12:00:00+05:30",
    );

    expect(rows[0]?.member.joined_at).toBe("2024-02-29T06:30:00.000Z");
  });

  it("preserves spreadsheet row numbers across blanks and multiline cells", () => {
    const rows = parseMembersCsv(
      'email,first_name,last_name,profile\r\n\r\na@b.com,A,B,"line one\r\nline two"\r\n\r\nc@d.com,C,D,',
    );

    expect(rows.map((row) => row.sourceRow)).toEqual([3, 5]);
    expect(rows[0]?.member.profile).toBe("line one\r\nline two");
  });

  it("uses the original spreadsheet row number in validation errors", () => {
    expect(() =>
      parseMembersCsv("email,first_name,last_name\n\ninvalid,A,B"),
    ).toThrow("Row 3: Enter a valid email");
  });

  it("rejects files larger than the advertised limit", () => {
    expect(() => parseMembersCsv("x".repeat(maxCsvBytes + 1))).toThrow(
      "1 MB or less",
    );
  });

  it("rejects a small CSV whose escaped JSON exceeds the HTTP request limit", () => {
    const csv = `email,first_name,last_name,profile\na@b.com,A,B,${"\u0000".repeat(360000)}`;

    expect(new TextEncoder().encode(csv).length).toBeLessThan(maxCsvBytes);
    expect(() => parseMembersCsv(csv)).toThrow(
      "expands beyond the import size limit",
    );
  });

  it("accepts a large CSV whose request stays inside the HTTP limit", () => {
    const csv = `email,first_name,last_name,profile\na@b.com,A,B,${"x".repeat(maxCsvBytes - 100)}`;
    const rows = parseMembersCsv(csv);

    expect(
      new TextEncoder().encode(
        JSON.stringify({ members: rows.map((row) => row.member) }),
      ).length,
    ).toBeLessThanOrEqual(maxBulkRequestBytes);
  });
});
