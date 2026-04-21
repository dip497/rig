export function shortSha(sha: string | null | undefined): string {
  if (!sha) return "—";
  return sha.slice(0, 7);
}

export function titleCase(s: string): string {
  return s.replace(/(^|[-_ ])(\w)/g, (_, _a, c) => " " + c.toUpperCase()).trim();
}
