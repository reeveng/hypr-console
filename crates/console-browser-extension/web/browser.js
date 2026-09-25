'use strict';

/* The half of it that is not in the page.
 *
 * A content script can walk a page and press what is on it, and it can do
 * nothing at all about the browser around it: the tab strip, the address bar
 * and the engine a question goes to are all chrome, and chrome is a pointer
 * and a small target. So the page asks for those here, in one place, and each
 * answer is the browser's own rather than a second copy kept by this desktop.
 */

const answers = {
  /* The engines the browser has, the one it uses first. There is no list of
     engines in this add-on: the settings panel's Web tab writes a policy that
     tells the browser which to default to, and this reads back whatever that
     came to. A list here would be the same choice made twice. */
  async engines() {
    const every = await browser.search.get();
    return every
      .map((engine) => ({ name: engine.name, here: !!engine.isDefault }))
      .sort((one, two) => Number(two.here) - Number(one.here));
  },

  async ask(said) {
    const asked = { query: said.query };
    if (said.engine) asked.engine = said.engine;
    await browser.search.search(asked);
    return true;
  },

  async find(said, from) {
    const answer = await browser.find.find(said.query, { tabId: from.tab.id });
    return { many: (answer && answer.count) || 0 };
  },

  async show(said, from) {
    await browser.find.highlightResults({ tabId: from.tab.id, rangeIndex: said.at });
    return true;
  },

  async unfind() {
    await browser.find.removeHighlighting();
    return true;
  },

  async tabs(_said, from) {
    const every = await browser.tabs.query({ currentWindow: true });
    return every.map((tab) => ({
      id: tab.id,
      title: tab.title,
      host: host(tab.url),
      here: from.tab ? tab.id === from.tab.id : !!tab.active,
    }));
  },

  async go(said) {
    await browser.tabs.update(said.id, { active: true });
    return true;
  },

  async new() {
    await browser.tabs.create({});
    return true;
  },

  async close(_said, from) {
    if (from.tab) await browser.tabs.remove(from.tab.id);
    return true;
  },

  /* The tab that was closed. Two presses apart on a strip no one can aim at
     is the whole reason closing one is offered at all. */
  async reopen() {
    await browser.sessions.restore();
    return true;
  },

  /* Behind what is being read rather than over it, which is what a new tab is
     for. The page says so afterwards, because a tab that opened somewhere out
     of sight and said nothing is a press that looks like it did nothing. */
  async open(said) {
    await browser.tabs.create({ url: said.url, active: false });
    return true;
  },

  /* The three that are not the browser answering a page at all, but this
     add-on reaching past what a page is allowed -- `around.js` is what that is
     and why it is allowed. They are asked for here with everything else, so a
     page has one place it asks and does not have to know which of its
     questions needed privileges to answer. */
  async address() {
    return browser.around.address();
  },

  async menu() {
    return browser.around.menu();
  },

  /* And the third, which is not the browser at all: the desktop's own
     keyboard, raised for a card that has a line to type into. */
  async keyboard() {
    return browser.around.keyboard();
  },
};

/* The bookmarks, said to the desktop rather than kept in here.
 *
 * A bookmark is a thing someone opens, and everything else a person opens on
 * this machine is in the menu and can be put on the home screen from it. So
 * they are written out as desktop entries -- `crates/console-bookmarks` is the
 * argument -- and then the menu, the home screen and the panel need to know
 * nothing about bookmarks at all.
 *
 * Walking the tree and watching it change is what an add-on is for, and is
 * here. Fetching the picture a page has and starting a program of ours is what
 * an add-on may not do, and is in `around.js`.
 *
 * A folder is not a bookmark and neither is a separator: what is handed over
 * is what has an address, and what is not a page -- a keyword search, a query
 * the browser keeps for itself -- is dropped at the other end, where what a
 * desktop entry can hold is decided.
 */
function under(nodes, found) {
  for (const node of nodes || []) {
    if (node.children) under(node.children, found);
    else if (node.url) found.push({ id: node.id, url: node.url, title: title(node) });
  }
  return found;
}

/* A tab, a line break or a stray space would be a second field or a second
   line to whatever reads this, so the title is one line before it leaves. */
function title(node) {
  return (node.title || '').replace(/\s+/gu, ' ').trim();
}

/* One run at a time, and one more afterwards if anything changed while it ran.
   Importing bookmarks fires an event per bookmark, and a program started per
   event is a thousand programs; this is the same list written twice instead. */
let telling = null;
let again = false;

function tell() {
  if (telling) {
    again = true;
    return;
  }

  telling = browser.bookmarks
    .getTree()
    .then((tree) => browser.around.bookmarks(under(tree, [])))
    .catch(() => null)
    .then(() => {
      telling = null;
      if (again) {
        again = false;
        tell();
      }
    });
}

browser.bookmarks.onCreated.addListener(tell);
browser.bookmarks.onChanged.addListener(tell);
browser.bookmarks.onRemoved.addListener(tell);

tell();

/* Every page that arrives from outside gets a jar of its own.
 *
 * A bookmark on the home screen, a question typed into the menu and a link in
 * the guide all reach the browser the same way -- `xdg-open`, a command line,
 * a tab -- and a command line cannot say which container to open in. So they
 * all landed in the one the browser starts with, which is the jar every other
 * page on this device also lands in: one machine, one person, and every site
 * she opens sharing a cookie store with every other.
 *
 * What happens here instead is what the Temporary Containers add-on does in
 * its automatic mode, in the add-on this desktop already ships rather than a
 * second one installed beside it. A signed add-on from a store would bring its
 * own preferences page -- a desk-sized surface on a machine with no pointer to
 * aim at it -- to configure a rule this desktop has already made.
 *
 * The rule is the whole of it: a navigation in a tab that is still in the
 * browser's own container is stopped and opened again in a container made for
 * it. A tab that has been moved is no longer in that container, so the only
 * navigation this catches is the first one of a tab nothing else claimed --
 * which is exactly a thing opened from outside. Links followed from there stay
 * where they are, because a tab hands its container to what it opens: one jar
 * per thing opened, not one per page, which is the difference between being
 * isolated and being logged out every time you click.
 *
 * The jar is named for where it went, because the browser draws that name on
 * the tab and `once: example.com` is something a person can read. The mark is
 * what tells ours from a container someone made on purpose.
 *
 * They are taken away when the last tab in one closes, and the sweep looks at
 * all of them rather than the one that just emptied: this desktop stops the
 * browser rather than the person doing it, so a session that went down hard
 * leaves jars behind, and the next tab anyone closes is when they go.
 */
const ONCE = 'once';

const THE_BROWSERS_OWN = 'firefox-default';

async function apart(tab, url) {
  const where = host(url);

  const made = await browser.contextualIdentities.create({
    name: where ? `${ONCE}: ${where}` : ONCE,
    color: 'pink',
    icon: 'fingerprint',
  });

  await browser.tabs.create({
    url,
    cookieStoreId: made.cookieStoreId,
    index: tab.index,
    active: tab.active,
    windowId: tab.windowId,
  });

  await browser.tabs.remove(tab.id);
}

browser.webRequest.onBeforeRequest.addListener(
  (asked) => {
    if (asked.tabId < 0) return {};

    return browser.tabs
      .get(asked.tabId)
      .then((tab) => {
        if (tab.cookieStoreId !== THE_BROWSERS_OWN) return {};

        /* The request is stopped only once the tab it is moving to is open.
           A browser where containers are switched off answers the making of
           one with a promise that breaks, and a page stopped on the strength
           of a tab that was never opened is a press that loads nothing at
           all -- so what that costs is the jar, and not the page. */
        return apart(tab, asked.url).then(
          () => ({ cancel: true }),
          () => ({}),
        );
      })
      .catch(() => ({}));
  },
  { urls: ['<all_urls>'], types: ['main_frame'] },
  ['blocking'],
);

async function swept() {
  const every = await browser.contextualIdentities.query({});
  const tabs = await browser.tabs.query({});
  const held = new Set(tabs.map((tab) => tab.cookieStoreId));

  for (const one of every) {
    if (!one.name.startsWith(ONCE)) continue;
    if (held.has(one.cookieStoreId)) continue;
    await browser.contextualIdentities.remove(one.cookieStoreId);
  }
}

browser.tabs.onRemoved.addListener(() => {
  swept().catch(() => null);
});

function host(url) {
  try {
    return new URL(url).host;
  } catch (_) {
    return '';
  }
}

browser.runtime.onMessage.addListener((said, from) => {
  const answer = answers[said && said.say];
  if (!answer) return undefined;
  /* Anything that goes wrong here goes wrong with no terminal under it, so it
     is answered with nothing rather than left as a promise the page waits on. */
  return answer(said, from).catch(() => null);
});
