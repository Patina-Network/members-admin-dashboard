export function validateEmail(value: string) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.trim()) ? null : (
      "Enter a valid email"
    );
}

export function validateSocialUsername(value: string) {
  return /[\s/@?#]/u.test(value.trim()) ?
      "Please enter a username without a URL, @, or spaces"
    : null;
}

export function validateDiscordId(value: string, username: string) {
  const id = value.trim();
  if (!id)
    return username.trim() ? "Discord ID is required with a username" : null;
  return (
      /^\d+$/.test(id) && BigInt(id) > 0n && BigInt(id) <= 18446744073709551615n
    ) ?
      null
    : "Enter a valid Discord ID";
}
