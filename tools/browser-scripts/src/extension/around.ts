import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import { pipe } from "effect/Function"

import type { Bookmark } from "./api"

interface BrowserWindow {
  focus(): void
  readonly gURLBar?: { select(): void }
  readonly PanelUI?: { show(): void }
}

interface DesktopProcess {
  readonly stdin: {
    write(text: string): Promise<unknown>
    close(): Promise<unknown>
  }
  wait(): Promise<unknown>
}

declare abstract class ExtensionAPI {}
declare const Services: {
  readonly wm: {
    getMostRecentWindow(kind: string): BrowserWindow | null
  }
}
declare const ChromeUtils: {
  importESModule(path: string): {
    readonly Subprocess: {
      call(options: {
        readonly command: string
        readonly arguments: readonly string[]
        readonly stdin?: "pipe"
      }): Promise<DesktopProcess>
    }
  }
}

const desktopProcess = (command: string, stdin?: "pipe") => Effect.tryPromise(() => {
  const { Subprocess } = ChromeUtils.importESModule("resource://gre/modules/Subprocess.sys.mjs")
  return Subprocess.call({ command, arguments: [], stdin })
})

const success = <A, E>(work: Effect.Effect<A, E>) => Effect.runPromise(pipe(
  work,
  Effect.as(true),
  Effect.orElseSucceed(() => false),
))

const browserWindow = () => Services.wm.getMostRecentWindow("navigator:browser")

const focusChrome = <Control>(
  select: (window: BrowserWindow) => Control | undefined,
  activate: (control: Control) => void,
) => Effect.runPromise(pipe(
  Effect.try(() => pipe(
    Option.fromNullishOr(browserWindow()),
    Option.flatMap((window) => pipe(
      Option.fromNullishOr(select(window)),
      Option.map((control) => {
        window.focus()
        activate(control)
        return true
      }),
    )),
    Option.getOrElse(() => false),
  )),
  Effect.orElseSucceed(() => false),
))

const picture = (url: string) => pipe(
  Effect.tryPromise(async () => {
    const answer = await fetch(`page-icon:${url}`)
    const bytes = new Uint8Array(await answer.arrayBuffer())
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("")
  }),
  Effect.orElseSucceed(() => ""),
)

const bookmarkIndex = (bookmarks: readonly Bookmark[]) => Effect.gen(function* () {
  const lines = yield* Effect.forEach(bookmarks, (bookmark) => pipe(
    picture(bookmark.url),
    Effect.map((icon) => [bookmark.id, bookmark.url, bookmark.title, icon].join("\t")),
  ))
  const running = yield* desktopProcess("/usr/local/bin/bookmarks-index", "pipe")
  yield* Effect.tryPromise(() => running.stdin.write(`${lines.join("\n")}\n`))
  yield* Effect.tryPromise(() => running.stdin.close())
  yield* Effect.tryPromise(() => running.wait())
})

// LibreWolf loads this experiment API into the privileged add-on sandbox.
Object.assign(globalThis, {
  around: class extends ExtensionAPI {
    getAPI() {
      return {
        around: {
          address: () => focusChrome((window) => window.gURLBar, (bar) => bar.select()),
          menu: () => focusChrome((window) => window.PanelUI, (panel) => panel.show()),
          bookmarks: (bookmarks: readonly Bookmark[]) => success(bookmarkIndex(bookmarks)),
          keyboard: () => success(desktopProcess("/usr/local/bin/keyboard-show")),
        },
      }
    }
  },
})
