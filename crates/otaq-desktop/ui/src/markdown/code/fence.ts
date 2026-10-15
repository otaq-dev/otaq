// How a code block is saved as Markdown (T-0254, Q48). Tiptap always writes a ``` fence, so a block whose code holds a
// ``` line (a Markdown example with its own code block) closed early and broke a little more on every save. Here the
// fence is longer than any run of backticks that starts a line of the code, so nothing inside can close it.

/** The longest run of backticks at the start of a line (after up to three spaces): the only runs that close a fence. */
function longestLeadingBackticks(code: string): number {
  let longest = 0;
  for (const match of code.matchAll(/^ {0,3}(`+)/gm)) {
    longest = Math.max(longest, match[1]?.length ?? 0);
  }
  return longest;
}

/** The fence for `code`: three backticks, or one more than the longest run that starts a line inside it. */
export function fenceFor(code: string): string {
  return "`".repeat(Math.max(3, longestLeadingBackticks(code) + 1));
}

/** A code block as Markdown: the fence with the language after it, the code, the fence again (as Tiptap writes it). */
export function codeBlockMarkdown(language: string | null | undefined, code: string): string {
  const fence = fenceFor(code);
  return `${fence}${language ?? ""}\n${code}\n${fence}`;
}
