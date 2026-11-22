import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import { pipe } from "effect/Function"

import { when } from "./decision"

const TEXT_LIMIT = 16_000
const BLOCKS = "h1, h2, h3, h4, p, li, blockquote, pre"
const NESTED = "p, li, blockquote, pre"
const IGNORED = "nav, aside, footer, form, button, script, style, noscript, [hidden], [aria-hidden=true]"

const articleRoot = (document: Document) => pipe(
  Option.fromNullishOr(document.querySelector<HTMLElement>('main article, [role="main"] article')),
  Option.orElse(() => Option.fromNullishOr(
    document.querySelector<HTMLElement>('article, main, [role="main"]'),
  )),
  Option.orElse(() => Option.fromNullishOr(document.body)),
)

const readable = (block: HTMLElement) =>
  block.closest(IGNORED) === null &&
  block.getClientRects().length > 0 &&
  block.querySelector(NESTED) === null

const extract = (root: HTMLElement, body: HTMLElement | null): string => {
  const sections: string[] = []

  let remaining = TEXT_LIMIT

  const blocks = root.querySelectorAll<HTMLElement>(BLOCKS).values()

  for (let block = blocks.next(); !block.done && remaining > 0; block = blocks.next()) {

    const text = pipe(
      Option.liftPredicate(readable)(block.value),
      Option.map((element) => element.innerText.trim().replace(/\s+/g, " ")),
      Option.filter((section) => section.length > 0),
    )

    for (const section of Option.toArray(text)) {
      sections.push(section.slice(0, remaining))
      remaining -= section.length + 2
    }
  }

  return pipe(
    when(sections.length === 0 && root !== body),
    Option.match({
      onSome: () => root.innerText.trim().slice(0, TEXT_LIMIT),
      onNone: () => sections.join("\n\n"),
    }),
  )
}

export const read = (document: Document): string => pipe(
  Effect.try(() => pipe(
    articleRoot(document),
    Option.map((root) => extract(root, document.body)),
    Option.getOrElse(() => ""),
  )),
  Effect.option,
  Effect.runSync,
  Option.getOrElse(() => ""),
)
