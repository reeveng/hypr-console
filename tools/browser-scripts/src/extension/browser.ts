import * as Match from "effect/Match"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import { pipe } from "effect/Function"

import { when } from "../decision"
import type { Bookmark, BookmarkNode, BrowserApi, Message, Sender, Tab } from "./api"

declare const browser: BrowserApi

const acknowledge = <A, E>(operation: Effect.Effect<A, E>) => Effect.as(operation, true)
const request = <A>(work: () => Promise<A>) => Effect.tryPromise(work)
const inTab = <A, E, B>(from: Sender, work: (tab: Tab) => Effect.Effect<A, E>, absent: B) =>
  pipe(Option.fromNullishOr(from.tab), Option.match({
    onNone: () => Effect.succeed<A | B>(absent),
    onSome: work,
  }))

const host = (url: string | undefined) => pipe(
  Option.fromNullishOr(url),
  Option.flatMap((address) => Option.liftThrowable(() => new URL(address).host)()),
  Option.getOrElse(() => ""),
)

// The extension asks the browser for its active engines, tabs, and find state.
const answer = (said: Message | null, from: Sender) => Match.value(said).pipe(
  Match.when({ say: "engines" }, () => pipe(
    request(() => browser.search.get()),
    Effect.map((all) => all.map((engine) => ({
      name: engine.name,
      here: Boolean(engine.isDefault),
    })).sort((one, two) => Number(two.here) - Number(one.here))),
  )),
  Match.when({ say: "ask" }, ({ query, engine }) => acknowledge(request(() =>
    browser.search.search({
      query,
      ...pipe(Option.fromNullishOr(engine), Option.filter((name) => name.length > 0),
        Option.map((name) => ({ engine: name })), Option.getOrElse(() => ({}))),
    })
  ))),
  Match.when({ say: "find" }, ({ query }) => inTab(from, (tab) => pipe(
    request(() => browser.find.find(query, { tabId: tab.id })),
    Effect.map((result) => ({ many: result?.count ?? 0 })),
  ), null)),
  Match.when({ say: "show" }, ({ at }) => inTab(from, (tab) => acknowledge(request(() =>
    browser.find.highlightResults({ tabId: tab.id, rangeIndex: at })
  )), null)),
  Match.when({ say: "unfind" }, () => acknowledge(request(() => browser.find.removeHighlighting()))),
  Match.when({ say: "tabs" }, () => pipe(
    request(() => browser.tabs.query({ currentWindow: true })),
    Effect.map((all) => all.map((tab) => ({
      id: tab.id,
      title: tab.title,
      host: host(tab.url),
      here: pipe(Option.fromNullishOr(from.tab), Option.match({
        onSome: (sender) => tab.id === sender.id,
        onNone: () => tab.active,
      })),
    }))),
  )),
  Match.when({ say: "go" }, ({ id }) => acknowledge(request(() => browser.tabs.update(id, { active: true })))),
  Match.when({ say: "new" }, () => acknowledge(request(() => browser.tabs.create({})))),
  Match.when({ say: "close" }, () =>
    inTab(from, (tab) => acknowledge(request(() => browser.tabs.remove(tab.id))), true)),
  Match.when({ say: "reopen" }, () => acknowledge(request(() => browser.sessions.restore()))),
  Match.when({ say: "open" }, ({ url }) => acknowledge(request(() => browser.tabs.create({ url, active: false })))),
  Match.when({ say: "address" }, () => request(() => browser.around.address())),
  Match.when({ say: "menu" }, () => request(() => browser.around.menu())),
  Match.when({ say: "keyboard" }, () => request(() => browser.around.keyboard())),
  Match.orElse(() => undefined),
)

browser.runtime.onMessage.addListener((said, from) => pipe(
  Option.fromNullishOr(answer(said, from)),
  Option.map((operation) => Effect.runPromise(Effect.orElseSucceed(operation, () => null))),
  Option.getOrUndefined,
))

// The desktop bookmark index reads lines from the extension's privileged API.
const under = (nodes: readonly BookmarkNode[]): Bookmark[] => nodes.flatMap((node) => pipe(
  Option.fromNullishOr(node.children),
  Option.match({
    onSome: under,
    onNone: () => pipe(
      Option.fromNullishOr(node.url),
      Option.filter((url) => url.length > 0),
      Option.map((url) => ({
        id: node.id,
        url,
        title: (node.title ?? "").replace(/\s+/gu, " ").trim(),
      })),
      Option.toArray,
    ),
  }),
))

// Events arriving during a write request one more pass.
let indexing: "idle" | "running" | "again" = "idle"

function tell(): void {
  Match.value(indexing).pipe(
    Match.whenOr("running", "again", () => { indexing = "again" }),
    Match.when("idle", () => {
      indexing = "running"
      const index = pipe(
        request(() => browser.bookmarks.getTree()),
        Effect.flatMap((tree) => request(() => browser.around.bookmarks(under(tree)))),
        Effect.ignore,
        Effect.ensuring(Effect.sync(() => {
          const finished = indexing
          indexing = "idle"
          Match.value(finished).pipe(
            Match.when("again", tell),
            Match.orElse(() => undefined),
          )
        })),
      )
      Effect.runFork(index)
    }),
    Match.exhaustive,
  )
}

browser.bookmarks.onCreated.addListener(tell)
browser.bookmarks.onChanged.addListener(tell)
browser.bookmarks.onRemoved.addListener(tell)
tell()

const ONCE = "once"
const DEFAULT_STORE = "firefox-default"

const apart = (tab: Tab, url: string) => Effect.gen(function* () {
  const where = host(url)
  const identity = yield* request(() => browser.contextualIdentities.create({
    name: pipe(when(where.length > 0), Option.match({
      onSome: () => `${ONCE}: ${where}`,
      onNone: () => ONCE,
    })),
    color: "pink",
    icon: "fingerprint",
  }))
  yield* request(() => browser.tabs.create({
    url,
    cookieStoreId: identity.cookieStoreId,
    index: tab.index,
    active: tab.active,
    windowId: tab.windowId,
  }))
  yield* request(() => browser.tabs.remove(tab.id))
  return { cancel: true }
})

// Cancel the original request only after its replacement tab is open.
browser.webRequest.onBeforeRequest.addListener(
  ({ tabId, url }) => pipe(when(tabId >= 0), Option.match({
    onNone: () => ({}),
    onSome: () => Effect.runPromise(pipe(
      request(() => browser.tabs.get(tabId)),
      Effect.flatMap((tab) => Match.value(tab.cookieStoreId).pipe(
        Match.when(DEFAULT_STORE, () => apart(tab, url)),
        Match.orElse(() => Effect.succeed({})),
      )),
      Effect.orElseSucceed(() => ({})),
    )),
  })),
  { urls: ["<all_urls>"], types: ["main_frame"] },
  ["blocking"],
)

const sweep = Effect.gen(function* () {
  const identities = yield* request(() => browser.contextualIdentities.query({}))
  const tabs = yield* request(() => browser.tabs.query({}))
  const held = new Set(tabs.map((tab) => tab.cookieStoreId))
  const empty = identities.filter((identity) =>
    identity.name.startsWith(ONCE) && !held.has(identity.cookieStoreId)
  )
  yield* Effect.forEach(empty, (identity) =>
    request(() => browser.contextualIdentities.remove(identity.cookieStoreId)),
  { discard: true })
})

browser.tabs.onRemoved.addListener(() => {
  Effect.runFork(Effect.ignore(sweep))
})
