export interface Bookmark {
  readonly id: string
  readonly title: string
  readonly url: string
}

export interface BookmarkNode extends Omit<Bookmark, "title" | "url"> {
  readonly title?: string
  readonly url?: string
  readonly children?: readonly BookmarkNode[]
}

export interface Tab {
  readonly id: number
  readonly index: number
  readonly windowId: number
  readonly active: boolean
  readonly title?: string
  readonly url?: string
  readonly cookieStoreId?: string
}

export interface Sender {
  readonly tab?: Tab
}

interface Arguments {
  engines: object
  ask: { readonly query: string; readonly engine?: string }
  find: { readonly query: string }
  show: { readonly at: number }
  unfind: object
  tabs: object
  go: { readonly id: number }
  new: object
  close: object
  reopen: object
  open: { readonly url: string }
  address: object
  menu: object
  keyboard: object
}

export interface Answers {
  engines: readonly { readonly name: string; readonly here: boolean }[]
  ask: boolean
  find: { readonly many: number }
  show: boolean
  unfind: boolean
  tabs: readonly { readonly id: number; readonly title?: string; readonly host: string; readonly here: boolean }[]
  go: boolean
  new: boolean
  close: boolean
  reopen: boolean
  open: boolean
  address: boolean
  menu: boolean
  keyboard: boolean
}

export type Message = {
  [Name in keyof Arguments]: { readonly say: Name } & Arguments[Name]
}[keyof Arguments]

export type MessageOf<Name extends keyof Answers> = Extract<Message, { say: Name }>

interface ChangeEvent {
  addListener(listener: () => void): void
}

interface Identity {
  readonly name: string
  readonly cookieStoreId: string
}

interface Guard {
  readonly cancel?: boolean
}

export interface BrowserApi {
  readonly search: {
    get(): Promise<readonly { readonly name: string; readonly isDefault?: boolean }[]>
    search(options: Arguments["ask"]): Promise<void>
  }
  readonly find: {
    find(query: string, options: { readonly tabId: number }): Promise<{ readonly count: number } | undefined>
    highlightResults(options: { readonly tabId: number; readonly rangeIndex: number }): Promise<void>
    removeHighlighting(): Promise<void>
  }
  readonly tabs: {
    get(id: number): Promise<Tab>
    query(options: { readonly currentWindow?: boolean }): Promise<readonly Tab[]>
    update(id: number, options: { readonly active: boolean }): Promise<Tab>
    create(options: Partial<Omit<Tab, "id">>): Promise<Tab>
    remove(id: number): Promise<void>
    readonly onRemoved: ChangeEvent
  }
  readonly sessions: { restore(): Promise<unknown> }
  readonly around: {
    address(): Promise<boolean>
    menu(): Promise<boolean>
    keyboard(): Promise<boolean>
    bookmarks(bookmarks: readonly Bookmark[]): Promise<boolean>
  }
  readonly bookmarks: {
    getTree(): Promise<readonly BookmarkNode[]>
    readonly onCreated: ChangeEvent
    readonly onChanged: ChangeEvent
    readonly onRemoved: ChangeEvent
  }
  readonly contextualIdentities: {
    create(options: { readonly name: string; readonly color: string; readonly icon: string }): Promise<Identity>
    query(options: Record<string, never>): Promise<readonly Identity[]>
    remove(id: string): Promise<unknown>
  }
  readonly webRequest: {
    readonly onBeforeRequest: {
      addListener(
        listener: (request: { readonly tabId: number; readonly url: string }) => Guard | Promise<Guard>,
        match: { readonly urls: readonly string[]; readonly types: readonly string[] },
        flags: readonly string[],
      ): void
    }
  }
  readonly runtime: {
    getURL(resource: string): string
    sendMessage<Name extends keyof Answers>(message: MessageOf<Name>): Promise<Answers[Name]>
    readonly onMessage: {
      addListener(listener: (message: Message | null, sender: Sender) => Promise<unknown> | undefined): void
    }
  }
}
