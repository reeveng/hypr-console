import * as Match from "effect/Match";
import * as Effect from "effect/Effect";
import * as Option from "effect/Option";
import { pipe } from "effect/Function";

import { when } from "../decision";
import type { Answers, BrowserApi, MessageOf } from "./api";

type Way = "u" | "r" | "d" | "l";
type HintMode = "take" | "new";
type PageElement = HTMLElement & { readonly disabled?: boolean };

interface PadLogic {
  readonly memory: WebAssembly.Memory;
  hint_label(count: number, index: number): bigint;
  geometry_buffer(count: number): number;
  place_hints(width: number, height: number): void;
  walk_toward(x: number, y: number, dx: number, dy: number, standing: number): number;
}

interface Item {
  readonly element: PageElement;
  readonly rect: DOMRect;
}

interface Mark {
  readonly label: string;
  readonly node: HTMLElement;
  readonly wrote: HTMLElement;
}

interface TargetMark extends Mark {
  readonly target: PageElement;
  readonly take?: never;
  rect: DOMRect;
}

interface DeedMark extends Mark {
  readonly target?: never;
  readonly take: () => unknown;
}

interface Hints {
  readonly mode: HintMode;
  readonly marks: readonly (TargetMark | DeedMark)[];
  readonly bar: HTMLElement;
  readonly wasm: PadLogic;
  typed: string;
}

interface CardRow {
  readonly key: string;
  readonly says: string;
  readonly aside?: string;
  readonly now?: boolean;
  readonly needs?: boolean;
  readonly does: (text: string) => unknown;
}

interface CardRequest {
  readonly title: string;
  readonly typing?: string;
  readonly note?: string;
  readonly standing?: string;
  rows(): Promise<readonly CardRow[]>;
}

interface Card {
  readonly node: HTMLElement;
  readonly list: HTMLElement;
  readonly field: HTMLInputElement | null;
  rows: { row: CardRow; node: HTMLElement }[];
  at: number;
}

interface PadState {
  host: HTMLDivElement | null;
  root: ShadowRoot | null;
  hints: Hints | null;
  standing: PageElement | null;
  card: Card | null;
  found: { readonly query: string; readonly many: number; at: number } | null;
  pointer: { readonly x: number; readonly y: number } | null;
  noting: number | undefined;
}

declare global {
  interface Window {
    __consolePad?: true;
  }
}

declare const browser: BrowserApi;

/* The pad, in a page.
 *
 * Everywhere else on this desktop the d-pad moves between things and A takes
 * the one it is standing on. A page was the exception: the stick pushed a
 * pointer at a link and A clicked wherever the pointer had got to, which is
 * the one place on the machine where getting somewhere was aiming rather than
 * choosing. This is that promise kept inside a page.
 *
 * Nothing here asks for a button of its own. The pad already sends a page four
 * things -- the arrows, Escape, a left click and a right click -- and every
 * word below is written in those four:
 *
 *   Y        the right click, which the guide calls "more options", and which
 *            here labels every single thing on the page that can be pressed
 *   D-pad    the arrows, which walk between those things, or type a label
 *   A        the left click, which takes what is being stood on rather than
 *            whatever the pointer happens to be over
 *   B        Escape, which puts a label away, then a card, then the highlight,
 *            and having nothing left to put away, goes back a page
 *
 * A label is written in arrows because arrows are what the d-pad sends. Vimium
 * writes them in letters and a letter costs the on-screen keyboard, which is
 * the slowest thing on this device and the whole reason this exists.
 *
 * The pointer is not taken away. It moves, the highlight goes, and where you
 * are is where your finger is -- the same answer the panels give a thumb.
 */

const WAYS: readonly Way[] = ["u", "r", "d", "l"]
const ARROWS: Readonly<Partial<Record<string, Way>>> = {
  ArrowUp: "u", ArrowRight: "r", ArrowDown: "d", ArrowLeft: "l",
}
const GLYPH: Readonly<Record<string, string>> = { u: "↑", r: "→", d: "↓", l: "←" }
const AXIS = { u: [0, -1], r: [1, 0], d: [0, 1], l: [-1, 0] } as const
const ROW_STEP = { u: -1, d: 1 } as const
const PRESSABLE = [
  "a[href]", "button", "input:not([type=hidden])", "select", "textarea", "summary",
  "label[for]", "video", "audio", "[role=button]", "[role=link]", "[role=tab]",
  "[role=checkbox]", "[role=radio]", "[role=switch]", "[role=menuitem]", "[role=option]",
  "[onclick]", '[contenteditable=""]', "[contenteditable=true]", '[tabindex]:not([tabindex="-1"])',
].join(",")
const BLOCKED_INPUTS = ["checkbox", "radio", "button", "submit", "reset", "file", "range", "color"]

const state: PadState = {
  host: null, root: null, hints: null, standing: null, card: null, found: null,
  pointer: null, noting: undefined,
}

function ask<Name extends keyof Answers>(
  said: { readonly say: Name } & MessageOf<Name>,
): Promise<Answers[Name] | null> {
  return Effect.runPromise(pipe(
    Effect.tryPromise(() => browser.runtime.sendMessage<Name>(said)),
    Effect.orElseSucceed(() => null),
  ))
}

function make<Tag extends keyof HTMLElementTagNameMap>(
  kind: Tag, className: string, text?: string,
): HTMLElementTagNameMap[Tag] {
  const node = document.createElement(kind)
  node.className = className
  node.textContent = text ?? null
  return node
}

function tap(glyph: string, about: string, does: () => unknown) {
  const button = make("button", "tap", glyph)
  button.title = about
  button.setAttribute("aria-label", about)
  button.addEventListener("click", (event) => {
    event.stopPropagation()
    does()
  })
  return button
}

let loadingSheets: Promise<readonly string[]> | undefined

function styles() {
  loadingSheets ??= Effect.runPromise(pipe(
    Effect.forEach(["palette.css", "pad.css"], (name) => Effect.tryPromise(async () => {
      const response = await fetch(browser.runtime.getURL(name))
      return response.text()
    }), { concurrency: "unbounded" }),
    Effect.tapError(() => Effect.sync(() => { loadingSheets = undefined })),
  ))
  return loadingSheets
}

function attached(root: ShadowRoot): ShadowRoot {
  pipe(
    Option.fromNullishOr(state.host),
    Option.filter((host) => !host.isConnected),
    Option.map((host) => (document.body ?? document.documentElement).appendChild(host)),
  )
  return root
}

function mount(sheets: readonly string[]) {
  const host = document.createElement("div")
  host.id = "console-pad"
  const root = host.attachShadow({ mode: "open" })
  // Constructed stylesheets work in pages with a strict content security policy.
  Effect.runSync(pipe(
    Effect.try(() => {
      root.adoptedStyleSheets = sheets.map((text) => {
        const sheet = new CSSStyleSheet()
        sheet.replaceSync(text)
        return sheet
      })
    }),
    Effect.orElseSucceed(() => {
      for (const text of sheets) root.appendChild(make("style", "", text))
    }),
  ))
  root.appendChild(make("div", "bars"))
  state.host = host
  state.root = root
  return attached(root)
}

async function surface(): Promise<ShadowRoot> {
  return pipe(Option.fromNullishOr(state.root), Option.match({
    onSome: attached,
    onNone: async () => {
      const sheets = await styles()
      return pipe(Option.fromNullishOr(state.root), Option.match({
        onSome: attached,
        onNone: () => mount(sheets),
      }))
    },
  }))
}

const bars = (root: ShadowRoot) => pipe(
  Option.fromNullishOr(root.querySelector(".bars")),
  Option.getOrThrow,
)

let padLogic: Promise<PadLogic> | undefined

function logic(): Promise<PadLogic> {
  padLogic ??= Effect.runPromise(pipe(
    Effect.tryPromise(() => fetch(browser.runtime.getURL("pad.wasm"))),
    Effect.filterOrFail((response) => response.ok,
      (response) => new Error("Cannot load pad.wasm: " + response.status)),
    Effect.flatMap((response) => Effect.tryPromise(() => response.arrayBuffer())),
    Effect.flatMap((bytes) => Effect.tryPromise(() => WebAssembly.instantiate(bytes))),
    Effect.map(({ instance }) => instance.exports as unknown as PadLogic),
    Effect.tapError(() => Effect.sync(() => { padLogic = undefined })),
  ))
  return padLogic
}

function labels(wasm: PadLogic, count: number) {
  return Array.from({ length: count }, (_, index) => {
    let bits = wasm.hint_label(count, index)
    let label = ""
    while (bits > 1n) {
      label = WAYS[Number(bits & 3n)] + label
      bits >>= 2n
    }
    return label
  })
}

function drawn(label: string, typed: string) {
  const node = make("span", "label")
  Array.from(label).forEach((way, index) => {
    const progress = pipe(when(index < typed.length), Option.match({
      onSome: () => "said", onNone: () => "owed",
    }))
    node.appendChild(make("b", progress, GLYPH[way]))
  })
  return node
}

function geometry(wasm: PadLogic, items: readonly { rect: DOMRect }[]) {
  const address = wasm.geometry_buffer(items.length)
  const fields = new Float64Array(wasm.memory.buffer, address, items.length * 6)
  items.forEach(({ rect }, index) => {
    fields.set([rect.left, rect.top, rect.width, rect.height], index * 6)
  })
  return fields
}

function settle(wasm: PadLogic, marks: readonly TargetMark[]) {
  const fields = geometry(wasm, marks)
  marks.forEach(({ node }, index) => {
    const { width, height } = node.getBoundingClientRect()
    fields.set([width, height], index * 6 + 4)
  })
  wasm.place_hints(innerWidth, innerHeight)
  marks.forEach(({ node }, index) => {
    node.style.left = fields[index * 6] + "px"
    node.style.top = fields[index * 6 + 1] + "px"
  })
}

const inViewport = (rect: DOMRect) =>
  rect.width >= 4 && rect.height >= 4 && rect.bottom > 0 && rect.right > 0 &&
  rect.top < innerHeight && rect.left < innerWidth

const visibleStyle = (element: PageElement) => {
  const style = getComputedStyle(element)
  return style.visibility === "visible" && style.display !== "none" && Number(style.opacity) !== 0
}

const seen = (element: PageElement | null) => pipe(
  Option.fromNullishOr(element),
  Option.filter((node) => node.isConnected && !node.disabled && node.getAttribute("aria-hidden") !== "true"),
  Option.flatMap((node) => pipe(
    Option.liftPredicate(inViewport)(node.getBoundingClientRect()),
    Option.filter(() => visibleStyle(node)),
  )),
)

function pressable(): Item[] {
  const found = Array.from(document.querySelectorAll<PageElement>(PRESSABLE)).flatMap((element) => pipe(
    seen(element),
    Option.filter(() => !state.host?.contains(element)),
    Option.map((rect) => ({ element, rect })),
    Option.toArray,
  ))
  // Prefer the inner target when a larger target covers the same place.
  return found.filter(({ element, rect }) => !found.some((other) =>
    other.element !== element && element.contains(other.element) &&
    Math.abs(other.rect.width - rect.width) < 6 && Math.abs(other.rect.height - rect.height) < 6
  )).sort((one, two) => one.rect.top - two.rect.top || one.rect.left - two.rect.left)
}

const deeds = () => [
  { says: "Look for something", does: () => raise(searching()) },
  { says: "Find on this page", does: () => raise(searching("page")) },
  { says: "The tabs", does: () => raise(tabbing()) },
  { says: "The address bar", does: () => ask({ say: "address" }) },
  { says: "The browser's menu", does: () => ask({ say: "menu" }) },
  { says: "A new tab", does: () => ask({ say: "new" }) },
  { says: "Close this tab", does: () => ask({ say: "close" }) },
  { says: "The tab that was closed", does: () => ask({ say: "reopen" }) },
  { says: "Load it again", does: () => location.reload() },
  { says: "The top of the page", does: () => scrollTo({ top: 0, behavior: "auto" }) },
]

const hintCopy = {
  take: { instruction: "Press what is written on it", dismiss: "Y for a new tab · B to put them away" },
  new: { instruction: "Open in a new tab", dismiss: "Y or B puts them away" },
}
const targetMark = (mark: TargetMark | DeedMark): mark is TargetMark => mark.target !== undefined

async function hints(mode: HintMode) {
  const [root, wasm] = await Promise.all([surface(), logic()])
  away()
  const items = Match.value(mode).pipe(
    Match.when("new", () => pressable().filter(({ element }) => element.closest("a[href]") !== null)),
    Match.when("take", pressable),
    Match.exhaustive,
  )
  const actions = Match.value(mode).pipe(
    Match.when("new", () => []),
    Match.when("take", deeds),
    Match.exhaustive,
  )
  const named = labels(wasm, actions.length + items.length)
  const copy = hintCopy[mode]
  const bar = make("div", "bar")
  bar.classList.toggle("new", mode === "new")
  bar.appendChild(make("span", "saying", copy.instruction))
  const actionMarks = actions.map(({ says, does }, index): DeedMark => {
    const label = named[index]
    const node = make("span", "deed")
    const wrote = drawn(label, "")
    node.append(wrote, make("span", "says", says))
    node.addEventListener("click", (event) => {
      event.stopPropagation()
      away()
      does()
    })
    bar.appendChild(node)
    return { label, node, wrote, take: does }
  })
  bar.append(make("span", "quiet", copy.dismiss), tap("×", "put the labels away", away))
  bars(root).appendChild(bar)
  const itemMarks = items.map(({ element, rect }, index): TargetMark => {
    const label = named[index + actions.length]
    const node = drawn(label, "")
    node.classList.add("hint")
    node.classList.toggle("new", mode === "new")
    root.appendChild(node)
    return { label, node, wrote: node, target: element, rect }
  })
  settle(wasm, itemMarks)
  state.hints = { mode, marks: [...actionMarks, ...itemMarks], typed: "", bar, wasm }
  unstand()
}

function away() {
  const previous = state.hints
  state.hints = null
  for (const hint of Option.toArray(Option.fromNullishOr(previous))) {
    hint.marks.filter(targetMark).forEach(({ node }) => node.remove())
    hint.bar.remove()
  }
}

function again() {
  Match.value(state.hints?.mode).pipe(
    Match.when("take", () => { void hints("new") }),
    Match.when("new", away),
    Match.orElse(() => undefined),
  )
}

function typed(hint: Hints, way: Way) {
  const proposed = hint.typed + way
  const matching = hint.marks.filter((mark) => mark.label.startsWith(proposed))
  hint.typed = pipe(when(matching.length > 0), Option.match({
    onSome: () => proposed, onNone: () => "",
  }))
  pipe(
    Option.fromNullishOr(matching.find((mark) => mark.label === hint.typed)),
    Option.match({
      onSome: (mark) => {
        away()
        Match.value(mark).pipe(
          Match.when(targetMark, (target) => take(target.target, hint.mode)),
          Match.orElse((deed) => { deed.take() }),
        )
      },
      onNone: () => hint.marks.forEach(({ label, node, wrote }) => {
        const shown = label.startsWith(hint.typed)
        node.classList.toggle("gone", !shown)
        for (const content of Option.toArray(Option.filter(Option.some(wrote), () => shown))) {
          content.replaceChildren(...drawn(label, hint.typed).childNodes)
        }
      }),
    }),
  )
}

const boxAt = (root: ShadowRoot) => pipe(
  Option.fromNullishOr(root.querySelector<HTMLElement>(".standing")),
  Option.getOrElse(() => root.appendChild(make("div", "standing"))),
)

function placeBox(box: HTMLElement, rect: DOMRect) {
  box.style.left = rect.left + "px"
  box.style.top = rect.top + "px"
  box.style.width = rect.width + "px"
  box.style.height = rect.height + "px"
}

async function walk(way: Way) {
  const [root, wasm] = await Promise.all([surface(), logic()])
  const from = seen(state.standing)
  const items = pressable()
  pipe(
    from,
    Option.match({
      onSome: (rect) => toward(wasm, rect, items, way),
      onNone: () => { unstand(); return items[0]?.element },
    }),
    Option.fromNullishOr,
    Option.match({
      onSome: (element) => {
        state.standing = element
        element.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "auto" })
        placeBox(boxAt(root), element.getBoundingClientRect())
        state.pointer = null
      },
      onNone: () => {
        const [x, y] = AXIS[way]
        scrollBy({ left: x * innerWidth * 0.4, top: y * innerHeight * 0.6, behavior: "auto" })
      },
    }),
  )
}

function toward(wasm: PadLogic, from: DOMRect, items: readonly Item[], way: Way) {
  const [dx, dy] = AXIS[way]
  geometry(wasm, items)
  const current = items.findIndex(({ element }) => element === state.standing)
  const next = wasm.walk_toward(from.left + from.width / 2, from.top + from.height / 2, dx, dy, current)
  return items[next]?.element
}

function unstand() {
  state.standing = null
  state.root?.querySelectorAll(".standing").forEach((box) => box.remove())
}

const editable = (node: Element | null): boolean =>
  node instanceof HTMLElement && (node.isContentEditable || node instanceof HTMLTextAreaElement ||
    (node instanceof HTMLInputElement && !BLOCKED_INPUTS.includes(node.type)))

const steered = (node: Element | null) => editable(node) || node instanceof HTMLSelectElement ||
  (node instanceof HTMLInputElement && node.type === "range")

const owns = (node: Element | null): node is HTMLElement =>
  node instanceof HTMLElement && (steered(node) || node instanceof HTMLVideoElement)

function take(element: PageElement, mode: HintMode) {
  const link = pipe(
    Option.fromNullishOr(element.closest<HTMLAnchorElement>("a[href]")?.href),
    Option.filter((url) => mode === "new" && url.length > 0),
  )
  pipe(link, Option.match({
    onSome: (url) => {
      void ask({ say: "open", url }).then(() => note("It is in a new tab, behind this one"))
    },
    onNone: () => Match.value(element.localName).pipe(
      Match.when(() => editable(element), () => {
        element.focus({ preventScroll: true })
        void ask({ say: "keyboard" })
        void note("Type. X puts the keyboard away, B leaves the field")
      }),
      Match.when(() => steered(element), () => {
        element.focus({ preventScroll: true })
        void note("The d-pad changes it, B when you are done")
      }),
      Match.orElse(() => {
        element.focus?.({ preventScroll: true })
        element.click?.()
      }),
    ),
  }))
}

async function raise(asked: CardRequest) {
  const root = await surface()
  away()
  shut()
  const node = make("div", "card")
  const top = make("div", "top")
  const out = make("button", "shut", "×")
  out.addEventListener("click", (event) => {
    event.stopPropagation()
    shut()
  })
  top.append(make("span", "title", asked.title), out)
  node.appendChild(top)
  const field = pipe(
    Option.fromNullishOr(asked.typing),
    Option.map((placeholder) => {
      const input = make("input", "field")
      input.type = "text"
      input.placeholder = placeholder
      node.appendChild(input)
      void ask({ say: "keyboard" })
      return input
    }),
    Option.getOrNull,
  )
  const list = make("div", "rows")
  node.appendChild(list)
  for (const text of Option.toArray(Option.fromNullishOr(asked.note))) {
    node.appendChild(make("div", "quiet", text))
  }
  root.appendChild(node)
  const card: Card = { node, list, field, rows: [], at: 0 }
  state.card = card
  const choices = await asked.rows()
  for (const current of Option.toArray(Option.filter(Option.some(card), () => state.card === card))) {
    rows(current, choices)
    field?.focus({ preventScroll: true })
    current.at = Math.max(0, choices.findIndex((choice) => choice.key === asked.standing))
    mark(current)
  }
}

function rows(card: Card, choices: readonly CardRow[]) {
  card.rows = choices.map((row, index) => {
    const node = make("div", "row")
    const label = pipe(when(row.now === true), Option.match({
      onSome: () => "now",
      onNone: () => "says",
    }))
    node.appendChild(make("span", label, row.says))
    for (const aside of Option.toArray(Option.filter(Option.fromNullishOr(row.aside), (text) => text.length > 0))) {
      node.appendChild(make("span", "aside", aside))
    }
    node.addEventListener("click", (event) => {
      event.stopPropagation()
      card.at = index
      chose(card)
    })
    return { row, node }
  })
  card.list.replaceChildren(...card.rows.map(({ node }) => node))
}

function mark(card: Card) {
  card.rows.forEach(({ node }, index) => node.classList.toggle("on", index === card.at))
  card.rows[card.at]?.node.scrollIntoView({ block: "nearest", behavior: "auto" })
}

function step(card: Card, by: number) {
  const count = card.rows.length
  for (const total of Option.toArray(Option.filter(Option.some(count), (size) => size > 0))) {
    card.at = (card.at + by + total) % total
    mark(card)
  }
}

function chose(card: Card) {
  const said = card.field?.value.trim() ?? ""
  for (const { row } of Option.toArray(Option.fromNullishOr(card.rows[card.at]))) {
    Match.value({ row, said }).pipe(
      Match.when(({ row, said }) => row.needs === true && said === "", () => {
        void note("Type what you are looking for first")
      }),
      Match.orElse(() => { row.does(said) }),
    )
  }
}

function shut() {
  state.card?.node.remove()
  state.card = null
}

function searching(standing?: string): CardRequest {
  return {
    title: "Look for something",
    typing: "What are you after?",
    note: "Type, then take the row you want. X puts the keyboard away.",
    standing,
    rows: async () => {
      const engines = await ask({ say: "engines" }) ?? []
      const choices: CardRow[] = engines.map((engine) => ({
        key: engine.name,
        says: engine.name,
        aside: pipe(when(engine.here), Option.map(() => "the usual one"), Option.getOrUndefined),
        now: engine.here,
        needs: true,
        does: (query) => {
          shut()
          void ask({ say: "ask", query, engine: engine.name })
        },
      }))
      const local = pipe(
        Option.liftPredicate((host: string) => host.length > 0)(location.host),
        Option.map((host): CardRow => ({
          key: "site",
          says: "On " + host,
          needs: true,
          does: (query) => {
            shut()
            void ask({ say: "ask", query: "site:" + host + " " + query })
          },
        })),
        Option.toArray,
      )
      return [...choices, ...local, {
        key: "page",
        says: "On this page",
        needs: true,
        does: (query) => {
          shut()
          void find(query)
        },
      }]
    },
  }
}

function tabbing(): CardRequest {
  return {
    title: "The tabs",
    note: "The one you are on is in mint.",
    rows: async () => (await ask({ say: "tabs" }) ?? []).map((tab) => ({
      key: String(tab.id),
      says: tab.title || tab.host || "a tab",
      aside: tab.host,
      now: tab.here,
      does: () => {
        shut()
        void ask({ say: "go", id: tab.id })
      },
    })),
  }
}

async function find(query: string) {
  const answer = await ask({ say: "find", query })
  await Match.value(answer?.many ?? 0).pipe(
    Match.when((many) => many > 0, async (many) => {
      state.found = { query, many, at: 0 }
      await ask({ say: "show", at: 0 })
      await saying()
    }),
    Match.orElse(() => note("Nothing on this page says " + query)),
  )
}

async function next(found: NonNullable<PadState["found"]>, by: number) {
  found.at = (found.at + by + found.many) % found.many
  await ask({ say: "show", at: found.at })
  await saying()
}

const byName = (root: ShadowRoot, name: "finding" | "noting") => pipe(
  Option.fromNullishOr(root.querySelector("." + name)),
  Option.getOrElse(() => bars(root).appendChild(make("div", "bar " + name))),
)

async function saying() {
  const root = await surface()
  for (const found of Option.toArray(Option.fromNullishOr(state.found))) {
    byName(root, "finding").replaceChildren(
      make("span", "saying", (found.at + 1) + " of " + found.many),
      make("span", "says", found.query),
      tap("↑", "the match before this one", () => next(found, -1)),
      tap("↓", "the next match", () => next(found, 1)),
      make("span", "quiet", "↑ ↓ for the next one · B when you are done"),
      tap("×", "done looking", unfind),
    )
  }
}

function unfind() {
  state.found = null
  void ask({ say: "unfind" })
  state.root?.querySelectorAll(".finding").forEach((bar) => bar.remove())
}

async function note(said: string) {
  const root = await surface()
  const bar = byName(root, "noting")
  bar.replaceChildren(make("span", "saying", said))
  clearTimeout(state.noting)
  state.noting = setTimeout(() => bar.remove(), 4000)
}

type Foreground =
  | { readonly kind: "hints"; readonly hint: Hints }
  | { readonly kind: "card"; readonly card: Card }
  | { readonly kind: "find"; readonly found: NonNullable<PadState["found"]> }
  | { readonly kind: "page" }

function foreground(): Foreground {
  return pipe(
    Option.fromNullishOr(state.hints),
    Option.map((hint) => ({ kind: "hints", hint } as const)),
    Option.orElse(() => pipe(
      Option.fromNullishOr(state.card),
      Option.map((card) => ({ kind: "card", card } as const)),
    )),
    Option.orElse(() => pipe(
      Option.fromNullishOr(state.found),
      Option.map((found) => ({ kind: "find", found } as const)),
    )),
    Option.getOrElse(() => ({ kind: "page" } as const)),
  )
}

function back() {
  Match.value(foreground().kind).pipe(
    Match.when("hints", away),
    Match.when("card", shut),
    Match.when("find", unfind),
    Match.when("page", () => pipe(
      Option.fromNullishOr(document.activeElement),
      Option.filter(owns),
      Option.match({
        onSome: (active) => active.blur(),
        onNone: () => pipe(Option.fromNullishOr(state.standing), Option.match({
          onSome: unstand,
          onNone: () => history.back(),
        })),
      }),
    )),
    Match.exhaustive,
  )
}

function swallow(event: Event) {
  event.preventDefault()
  event.stopPropagation()
}

const isRowKey = (way: Way): way is "u" | "d" => way === "u" || way === "d"

function arrow(event: KeyboardEvent, way: Way) {
  const front = foreground()
  Match.value(front.kind).pipe(
    Match.when("hints", () => {
      swallow(event)
      for (const hint of Option.toArray(Option.fromNullishOr(state.hints))) typed(hint, way)
    }),
    Match.when("card", () => pipe(Option.fromNullishOr(state.card), Option.map((card) => {
      Match.value(way).pipe(
        Match.when(isRowKey, (row) => { swallow(event); step(card, ROW_STEP[row]) }),
        Match.orElse(() => undefined),
      )
    }))),
    Match.when("find", () => pipe(Option.fromNullishOr(state.found), Option.map((found) => {
      Match.value(way).pipe(
        Match.when(isRowKey, (row) => { swallow(event); void next(found, ROW_STEP[row]) }),
        Match.orElse(() => undefined),
      )
    }))),
    Match.when("page", () => {
      const active = document.activeElement
      pipe(when(active === state.host || !owns(active)), Option.map(() => {
        swallow(event)
        void walk(way)
      }))
    }),
    Match.exhaustive,
  )
}

function keydown(event: KeyboardEvent) {
  Match.value(event.key).pipe(
    Match.when(() => event.altKey || event.ctrlKey || event.metaKey, () => undefined),
    Match.when("Escape", () => pipe(when(document.fullscreenElement === null), Option.map(() => {
      swallow(event)
      back()
    }))),
    Match.when("Enter", () => pipe(Option.fromNullishOr(state.card), Option.map((card) => {
      swallow(event)
      chose(card)
    }))),
    Match.orElse((key) => pipe(
      Option.fromNullishOr(ARROWS[key]),
      Option.map((way) => arrow(event, way)),
    )),
  )
}

function contextMenu(event: MouseEvent & { mozInputSource?: number }) {
  // A touch long-press still opens the browser's usual context menu.
  Match.value(event.mozInputSource).pipe(
    Match.when(5, () => undefined),
    Match.orElse(() => {
      swallow(event)
      Match.value(foreground().kind).pipe(
        Match.when("card", () => undefined),
        Match.when("hints", again),
        Match.orElse(() => { void hints("take") }),
      )
    }),
  )
}

function click(event: MouseEvent) {
  const path = event.composedPath()
  Match.value(foreground().kind).pipe(
    Match.when(() => state.host !== null && path.includes(state.host), () => undefined),
    Match.when("card", () => pipe(Option.fromNullishOr(state.card), Option.map((card) => {
      swallow(event)
      chose(card)
    }))),
    Match.when("hints", () => swallow(event)),
    Match.orElse(() => pipe(Option.fromNullishOr(state.standing), Option.map((standing) => {
      swallow(event)
      unstand()
      take(standing, "take")
    }))),
  )
}

function mousemove(event: MouseEvent) {
  const point = { x: event.clientX, y: event.clientY }
  pipe(
    Option.fromNullishOr(state.pointer),
    Option.filter(() => state.standing !== null),
    Option.match({
      onSome: (pointer) => {
        const distance = Math.abs(point.x - pointer.x) + Math.abs(point.y - pointer.y)
        Match.value(distance).pipe(Match.when((far) => far > 12, unstand), Match.orElse(() => undefined))
      },
      onNone: () => { state.pointer = point },
    }),
  )
}

let movement: "ready" | "queued" = "ready"

function moved() {
  Match.value(movement).pipe(
    Match.when("ready", () => {
      movement = "queued"
      requestAnimationFrame(() => {
        movement = "ready"
        for (const hint of Option.toArray(Option.fromNullishOr(state.hints))) {
          const visible = hint.marks.filter(targetMark).flatMap((mark) => {
            const rect = seen(mark.target)
            mark.node.classList.toggle("gone", Option.isNone(rect))
            return pipe(rect, Option.map((bounds) => { mark.rect = bounds; return mark }), Option.toArray)
          })
          settle(hint.wasm, visible)
        }
        for (const box of Option.toArray(Option.fromNullishOr(state.root?.querySelector<HTMLElement>(".standing")))) {
          pipe(seen(state.standing), Option.match({ onSome: (rect) => placeBox(box, rect), onNone: unstand }))
        }
      })
    }),
    Match.when("queued", () => undefined),
    Match.exhaustive,
  )
}

function start() {
  window.__consolePad = true
  window.addEventListener("keydown", keydown, true)
  window.addEventListener("contextmenu", contextMenu, true)
  window.addEventListener("click", click, true)
  window.addEventListener("mousemove", mousemove, true)
  window.addEventListener("scroll", moved, true)
  window.addEventListener("resize", moved, true)

  Match.value([document.documentElement.dataset.console, document.readyState] as const).pipe(
    Match.when(["new", "loading"], () => {
      window.addEventListener("DOMContentLoaded", () => { void raise(searching()) }, { once: true })
    }),
    Match.when(["new", Match.any], () => { void raise(searching()) }),
    Match.orElse(() => undefined),
  )
}

pipe(Option.fromNullishOr(window.__consolePad), Option.match({
  onNone: start,
  onSome: () => undefined,
}))
