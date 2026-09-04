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

  /* The tab that was closed. Two presses apart on a strip nobody can aim at
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
