/** The "doc" of an MCP tool reply (some answer JSON with a markdown doc), or the text itself. */
export const docOf = (text: string) => {
  try {
    const v = JSON.parse(text);
    if (typeof v?.doc === "string") return v.doc as string;
    return typeof v === "string" ? v : "```\n" + JSON.stringify(v, null, 2) + "\n```";
  } catch {
    return text;
  }
};
