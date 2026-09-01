'use strict';

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

(() => {
  if (window.__consolePad) return;
  window.__consolePad = true;

  /* ---------------------------------------------------------------- arrows */

  /** The four, in the order a label counts through them. */
  const WAYS = ['u', 'r', 'd', 'l'];
  const OF = { ArrowUp: 'u', ArrowRight: 'r', ArrowDown: 'd', ArrowLeft: 'l' };
  const GLYPH = { u: '↑', r: '→', d: '↓', l: '←' };

  /** Which way each is, as a step across the page. */
  const AXIS = { u: [0, -1], r: [1, 0], d: [0, 1], l: [-1, 0] };

  /* Everything a page offers a thumb. Wider than a list of links, because half
     of what is pressed on a page today is a div that was given a job in
     JavaScript, and narrower than everything, because a label over a thing
     that does nothing is a label that has to be read and then ignored. */
  const PRESSABLE = [
    'a[href]',
    'button',
    'input:not([type=hidden])',
    'select',
    'textarea',
    'summary',
    'label[for]',
    'video',
    'audio',
    '[role=button]',
    '[role=link]',
    '[role=tab]',
    '[role=checkbox]',
    '[role=radio]',
    '[role=switch]',
    '[role=menuitem]',
    '[role=option]',
    '[onclick]',
    '[contenteditable=""]',
    '[contenteditable=true]',
    '[tabindex]:not([tabindex="-1"])',
  ].join(',');

  /** How far the pointer has to move before the highlight gives way to it. */
  const DRIFT = 12;

  const state = {
    host: null,
    root: null,
    hints: null,
    standing: null,
    card: null,
    found: null,
    pointer: null,
    noting: null,
  };

  /* ------------------------------------------------------------------ paint */

  /* Ours is drawn in a shadow root, which is the only way to put something on
     somebody else's page and be sure their stylesheet cannot reach it and ours
     cannot reach theirs. The two sheets are adopted rather than linked: a page
     with a strict policy about where a stylesheet may come from is entitled to
     that policy, and a constructed sheet is not a fetch it can refuse. */
  let sheets = null;

  async function surface() {
    if (state.root) return attached(state.root);
    /* The sheets first, and the surface after them. Every measurement here is
       in the stylesheet, so a surface put up before its sheet had arrived
       would be drawn once at whatever size the page happens to be set in. */
    if (!sheets) {
      sheets = await Promise.all(
        ['palette.css', 'pad.css'].map((named) =>
          fetch(browser.runtime.getURL(named)).then((it) => it.text()),
        ),
      );
    }
    if (state.root) return attached(state.root);
    state.host = document.createElement('div');
    state.host.id = 'console-pad';
    state.root = state.host.attachShadow({ mode: 'open' });
    dressed(state.root);
    /* Everything that is a bar goes in one column at the bottom, so a page
       being searched and a page wearing labels are two bars above each other
       rather than two bars in the same place. */
    state.root.appendChild(make('div', 'bars'));
    return attached(state.root);
  }

  /* A constructed sheet is not a fetch, so a page with a strict policy about
     where a stylesheet may come from cannot refuse it -- and that policy is a
     page's to have. Where one cannot be constructed at all the text goes in as
     it is, which is the same sheet by the longer road. */
  function dressed(root) {
    try {
      root.adoptedStyleSheets = sheets.map((said) => {
        const sheet = new CSSStyleSheet();
        sheet.replaceSync(said);
        return sheet;
      });
    } catch (_) {
      for (const said of sheets) root.appendChild(make('style', '', said));
    }
  }

  /* A page that replaces its own body -- which is most of what a site does
     when it moves from one page to the next without loading one -- takes ours
     out with it. */
  function attached(root) {
    if (state.host && !state.host.isConnected) {
      (document.body || document.documentElement).appendChild(state.host);
    }
    return root;
  }

  /** The column at the bottom that every bar is drawn in. */
  function bars() {
    return state.root.querySelector('.bars');
  }

  function make(kind, className, text) {
    const node = document.createElement(kind);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  }

  /** A label, drawn as the arrows it is, with what has been pressed dimmed. */
  function drawn(label, typed) {
    const node = make('span', 'label');
    for (let at = 0; at < label.length; at += 1) {
      node.appendChild(make('b', at < typed.length ? 'said' : 'owed', GLYPH[label[at]]));
    }
    return node;
  }

  /* ------------------------------------------------------------------ labels */

  /* A label is prefix-free, so a press either finishes one or narrows the list,
     and never both. The short ones are handed out first and go to the deeds
     along the bottom, which is where the pressing actually is: on a page with
     five links, three of them are one press. */
  function labels(many) {
    if (many <= 0) return [];
    const base = WAYS.length;
    let length = 1;
    while (Math.pow(base, length) < many) length += 1;
    const stems = strings(length - 1);
    const short = length > 1 ? Math.min(stems.length, Math.floor((Math.pow(base, length) - many) / (base - 1))) : 0;
    const out = stems.slice(0, short);
    for (const stem of stems.slice(short)) for (const way of WAYS) out.push(stem + way);
    return out.slice(0, many);
  }

  function strings(length) {
    let out = [''];
    for (let at = 0; at < length; at += 1) out = out.flatMap((stem) => WAYS.map((way) => stem + way));
    return out;
  }

  /* --------------------------------------------------------------- the page */

  /** Whether a thing is on the screen at all, and where. */
  function seen(el) {
    if (!el || !el.isConnected) return null;
    if (el.disabled || el.getAttribute('aria-hidden') === 'true') return null;
    const rect = el.getBoundingClientRect();
    if (rect.width < 4 || rect.height < 4) return null;
    if (rect.bottom <= 0 || rect.right <= 0) return null;
    if (rect.top >= innerHeight || rect.left >= innerWidth) return null;
    const style = getComputedStyle(el);
    if (style.visibility !== 'visible' || style.display === 'none' || Number(style.opacity) === 0) return null;
    return rect;
  }

  /** Everything on the screen that can be pressed, nearest the top first. */
  function pressable() {
    const found = [];
    for (const el of document.querySelectorAll(PRESSABLE)) {
      if (state.host && state.host.contains(el)) continue;
      const rect = seen(el);
      if (!rect) continue;
      found.push({ el, rect });
    }
    /* A link wrapped round a button is one thing to press and two things
       found. The inner one is the one a page means, so an outer one standing
       in the same place is dropped. */
    const kept = found.filter(({ el, rect }) =>
      !found.some(
        (other) =>
          other.el !== el &&
          el.contains(other.el) &&
          Math.abs(other.rect.width - rect.width) < 6 &&
          Math.abs(other.rect.height - rect.height) < 6,
      ),
    );
    kept.sort((one, two) => one.rect.top - two.rect.top || one.rect.left - two.rect.left);
    return kept;
  }

  /* ----------------------------------------------------------------- deeds */

  /* What the browser can be asked for from inside a page. Each of these is
     somewhere the pad cannot otherwise reach: the tab strip, the address bar
     and the browser's own menus are all chrome, and chrome is a pointer and a
     small target. They are laid out along the bottom with the labels, so what
     can be done is on the screen rather than in somebody's memory. */
  function deeds() {
    return [
      { says: 'Look for something', does: () => raise(searching()) },
      { says: 'Find on this page', does: () => raise(searching('page')) },
      { says: 'The tabs', does: () => raise(tabbing()) },
      { says: 'A new tab', does: () => ask({ say: 'new' }) },
      { says: 'Close this tab', does: () => ask({ say: 'close' }) },
      { says: 'The tab that was closed', does: () => ask({ say: 'reopen' }) },
      { says: 'Load it again', does: () => location.reload() },
      { says: 'The top of the page', does: () => scrollTo({ top: 0, behavior: 'auto' }) },
    ];
  }

  /* ----------------------------------------------------------------- hints */

  /** Y: label everything, and say what taking one will do. */
  async function hints(mode) {
    const root = await surface();
    away();
    const every = pressable();
    const marks = [];
    const said = labels(deeds().length + every.length);
    let at = 0;

    const bar = make('div', 'bar');
    bar.appendChild(make('span', 'saying', mode === 'new' ? 'Open in a new tab' : 'Press what is written on it'));
    for (const deed of deeds()) {
      const item = make('span', 'deed');
      item.appendChild(drawn(said[at], ''));
      item.appendChild(make('span', 'says', deed.says));
      /* And the same thing for a hand holding nothing. Every button on this
         desktop has an answer for a finger, and a row of things that can only
         be reached by pressing arrows would be the first that does not. */
      item.addEventListener('click', (event) => {
        event.stopPropagation();
        away();
        deed.does();
      });
      bar.appendChild(item);
      marks.push({ label: said[at], node: item, wrote: item.firstChild, take: deed.does });
      at += 1;
    }
    bar.appendChild(
      make('span', 'quiet', mode === 'new' ? 'Y or B puts them away' : 'Y for a new tab · B to put them away'),
    );
    bars().appendChild(bar);

    for (const { el, rect } of every) {
      const node = drawn(said[at], '');
      node.classList.add('hint');
      place(node, rect);
      root.appendChild(node);
      marks.push({ label: said[at], node, wrote: node, target: el, rect });
      at += 1;
    }

    state.hints = { mode, marks, typed: '', bar };
    unstand();
  }

  function place(node, rect) {
    node.style.left = `${Math.max(2, Math.min(innerWidth - 96, rect.left))}px`;
    node.style.top = `${Math.max(2, Math.min(innerHeight - 36, rect.top))}px`;
  }

  /** One press of the d-pad, while the labels are up. */
  function typed(way) {
    const hint = state.hints;
    hint.typed += way;
    let left = hint.marks.filter((mark) => mark.label.startsWith(hint.typed));
    if (!left.length) {
      hint.typed = '';
      left = hint.marks;
    }
    const done = left.find((mark) => mark.label === hint.typed);
    if (done) {
      const mode = hint.mode;
      away();
      if (done.take) done.take();
      else take(done.target, mode);
      return;
    }
    for (const mark of hint.marks) {
      const matching = mark.label.startsWith(hint.typed);
      mark.node.classList.toggle('gone', !matching);
      if (!matching) continue;
      mark.wrote.replaceChildren(...drawn(mark.label, hint.typed).childNodes);
    }
  }

  /** Y again: the same labels, and what they will do said differently. */
  function again() {
    const mode = state.hints.mode;
    if (mode === 'take') hints('new');
    else away();
  }

  function away() {
    if (!state.hints) return;
    for (const mark of state.hints.marks) if (mark.target) mark.node.remove();
    state.hints.bar.remove();
    state.hints = null;
  }

  /* ------------------------------------------------------------- the walk */

  /* The d-pad between the things on a page, which is what the d-pad means
     everywhere else on this machine. Nothing is written into the page to show
     it: the box is drawn in our own layer over where the thing is, so a page
     that styles its own focus goes on styling it and nothing of ours is left
     behind on a page that outlives us. */
  async function walk(way) {
    const root = await surface();
    if (state.standing && !seen(state.standing)) unstand();
    const every = pressable();
    const from = state.standing ? state.standing.getBoundingClientRect() : null;
    const next = from ? towards(from, every, way) : nearest(every);
    if (!next) return sail(way);
    state.standing = next;
    let box = root.querySelector('.standing');
    if (!box) {
      box = make('div', 'standing');
      root.appendChild(box);
    }
    next.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'auto' });
    const rect = next.getBoundingClientRect();
    box.style.left = `${rect.left}px`;
    box.style.top = `${rect.top}px`;
    box.style.width = `${rect.width}px`;
    box.style.height = `${rect.height}px`;
    state.pointer = null;
  }

  function nearest(every) {
    return every.length ? every[0].el : null;
  }

  /** The thing that way, which is the near one that is also the aligned one. */
  function towards(from, every, way) {
    const axis = AXIS[way];
    let best = null;
    let cost = Infinity;
    for (const { el, rect } of every) {
      if (el === state.standing) continue;
      const dx = rect.left + rect.width / 2 - (from.left + from.width / 2);
      const dy = rect.top + rect.height / 2 - (from.top + from.height / 2);
      const along = dx * axis[0] + dy * axis[1];
      const across = Math.abs(dx * axis[1] + dy * axis[0]);
      if (along < 4) continue;
      const asked = along + across * 3;
      if (asked < cost) {
        cost = asked;
        best = el;
      }
    }
    return best;
  }

  /* Nothing that way is not a dead button. The page moves instead, which is
     what a thumb pressing down at the bottom of a list means by it. */
  function sail(way) {
    const axis = AXIS[way];
    scrollBy({
      left: axis[0] * innerWidth * 0.4,
      top: axis[1] * innerHeight * 0.6,
      behavior: 'auto',
    });
  }

  function unstand() {
    state.standing = null;
    if (state.root) state.root.querySelectorAll('.standing').forEach((box) => box.remove());
  }

  /* ------------------------------------------------------------------ take */

  function editable(el) {
    if (!el) return false;
    if (el.isContentEditable) return true;
    const kind = (el.tagName || '').toLowerCase();
    if (kind === 'textarea') return true;
    if (kind !== 'input') return false;
    return !['checkbox', 'radio', 'button', 'submit', 'reset', 'file', 'range', 'color'].includes(el.type);
  }

  /* Taken, these want the focus and not a click. A line is typed into, a list
     is chosen from and a level is moved, and all three of those are the arrows
     rather than a press. */
  function steered(el) {
    if (editable(el)) return true;
    const kind = ((el && el.tagName) || '').toLowerCase();
    return kind === 'select' || (kind === 'input' && el.type === 'range');
  }

  /* What the arrows belong to once it has the focus. The d-pad walks the page
     everywhere except inside one of these, where walking away from a thing the
     moment it was taken is the press undoing itself. A video is here and not
     above it: taking one is a click, which is what plays it, and the arrows
     are what a person wants next. */
  function owns(el) {
    if (steered(el)) return true;
    return ((el && el.tagName) || '').toLowerCase() === 'video';
  }

  function take(el, mode) {
    if (!el) return;
    if (mode === 'new') {
      const link = el.closest ? el.closest('a[href]') : null;
      if (link && link.href) {
        ask({ say: 'open', url: link.href }).then(() => note('It is in a new tab, behind this one'));
        return;
      }
    }
    if (steered(el)) {
      el.focus({ preventScroll: true });
      note(editable(el) ? 'Press X for the keyboard' : 'The d-pad changes it, B when you are done');
      return;
    }
    if (el.focus) el.focus({ preventScroll: true });
    el.click();
  }

  /* ------------------------------------------------------------------ cards */

  /* The same card the rest of the desktop draws: a list of rows, the one you
     are standing on in pink, and a line at the top to type into. It is the
     same shape on purpose. Somebody who has used the menu has used this. */
  async function raise(asked) {
    const root = await surface();
    away();
    shut();
    const node = make('div', 'card');
    const top = make('div', 'top');
    top.appendChild(make('span', 'title', asked.title));
    const out = make('button', 'shut', '\u00d7');
    out.addEventListener('click', (event) => {
      event.stopPropagation();
      shut();
    });
    top.appendChild(out);
    node.appendChild(top);
    let field = null;
    if (asked.typing) {
      field = make('input', 'field');
      field.type = 'text';
      field.placeholder = asked.typing;
      node.appendChild(field);
    }
    const list = make('div', 'rows');
    node.appendChild(list);
    if (asked.note) node.appendChild(make('div', 'quiet', asked.note));
    root.appendChild(node);
    state.card = { asked, node, list, field, rows: [], at: 0 };
    rows(await asked.rows());
    if (field) field.focus({ preventScroll: true });
    if (asked.standing !== undefined) {
      state.card.at = Math.max(0, state.card.rows.findIndex((row) => row.key === asked.standing));
      mark();
    }
  }

  function rows(said) {
    const card = state.card;
    if (!card) return;
    card.rows = said;
    card.list.replaceChildren();
    said.forEach((row, at) => {
      const node = make('div', 'row');
      node.appendChild(make('span', row.now ? 'now' : 'says', row.says));
      if (row.aside) node.appendChild(make('span', 'aside', row.aside));
      node.addEventListener('click', (event) => {
        event.stopPropagation();
        card.at = at;
        chose();
      });
      card.list.appendChild(node);
      row.node = node;
    });
    card.at = Math.min(card.at, Math.max(0, said.length - 1));
    mark();
  }

  function mark() {
    const card = state.card;
    if (!card) return;
    card.rows.forEach((row, at) => row.node.classList.toggle('on', at === card.at));
    const on = card.rows[card.at];
    if (on) on.node.scrollIntoView({ block: 'nearest', behavior: 'auto' });
  }

  function step(by) {
    const card = state.card;
    if (!card || !card.rows.length) return;
    card.at = (card.at + by + card.rows.length) % card.rows.length;
    mark();
  }

  function chose() {
    const card = state.card;
    if (!card) return;
    const row = card.rows[card.at];
    if (!row) return;
    const said = card.field ? card.field.value.trim() : '';
    if (row.needs && !said) return note('Press X for the keyboard');
    row.does(said);
  }

  function shut() {
    if (!state.card) return;
    state.card.node.remove();
    state.card = null;
  }

  /* ---------------------------------------------------------------- asking */

  /* Where a question goes is the browser's own answer, not a second one kept
     here. The engines listed are the engines the browser has, and the one at
     the top is the one this desktop chose on the settings panel's Web tab: an
     engine written down here would be the same choice made twice and wrong
     the first day somebody changed it. */
  function searching(standing) {
    return {
      title: 'Look for something',
      typing: 'What are you after?',
      note: 'Press X for the keyboard, then the row you want.',
      standing: standing,
      rows: async () => {
        const engines = (await ask({ say: 'engines' })) || [];
        const found = engines.map((engine) => ({
          key: engine.name,
          says: engine.name,
          aside: engine.here ? 'the usual one' : '',
          now: engine.here,
          needs: true,
          does: (said) => {
            shut();
            ask({ say: 'ask', query: said, engine: engine.name });
          },
        }));
        if (location.host) {
          found.push({
            key: 'site',
            says: `On ${location.host}`,
            needs: true,
            does: (said) => {
              shut();
              ask({ say: 'ask', query: `site:${location.host} ${said}` });
            },
          });
        }
        found.push({
          key: 'page',
          says: 'On this page',
          needs: true,
          does: (said) => {
            shut();
            find(said);
          },
        });
        return found;
      },
    };
  }

  function tabbing() {
    return {
      title: 'The tabs',
      note: 'The one you are on is in mint.',
      rows: async () => {
        const tabs = (await ask({ say: 'tabs' })) || [];
        return tabs.map((tab) => ({
          key: String(tab.id),
          says: tab.title || tab.host || 'a tab',
          aside: tab.host,
          now: tab.here,
          does: () => {
            shut();
            ask({ say: 'go', id: tab.id });
          },
        }));
      },
    };
  }

  /* ------------------------------------------------------------------ find */

  /* The browser's own find, driven from the pad: what was found stays on the
     screen and the d-pad steps through it, because a match nobody can walk to
     is a count rather than an answer. */
  async function find(said) {
    const answer = await ask({ say: 'find', query: said });
    const many = (answer && answer.many) || 0;
    if (!many) return note(`Nothing on this page says ${said}`);
    state.found = { query: said, many, at: 0 };
    await ask({ say: 'show', at: 0 });
    saying();
  }

  async function next(by) {
    if (!state.found) return;
    state.found.at = (state.found.at + by + state.found.many) % state.found.many;
    await ask({ say: 'show', at: state.found.at });
    saying();
  }

  async function saying() {
    const root = await surface();
    let bar = root.querySelector('.finding');
    if (!bar) {
      bar = make('div', 'bar finding');
      bars().appendChild(bar);
    }
    bar.replaceChildren(
      make('span', 'saying', `${state.found.at + 1} of ${state.found.many}`),
      make('span', 'says', state.found.query),
      make('span', 'quiet', '↑ ↓ for the next one · B when you are done'),
    );
  }

  function unfind() {
    state.found = null;
    ask({ say: 'unfind' });
    if (state.root) state.root.querySelectorAll('.finding').forEach((bar) => bar.remove());
  }

  /* ------------------------------------------------------------------ notes */

  /* Said on the screen rather than in the console, for the reason the whole
     desktop says things on the screen: there is no terminal under a thumb. */
  async function note(said) {
    const root = await surface();
    let bar = root.querySelector('.noting');
    if (!bar) {
      bar = make('div', 'bar noting');
      bars().appendChild(bar);
    }
    bar.replaceChildren(make('span', 'saying', said));
    clearTimeout(state.noting);
    state.noting = setTimeout(() => bar.remove(), 4000);
  }

  /* ------------------------------------------------------------------ going */

  /* B, which is one button and a list, in the order somebody would undo what
     they just did. Having nothing of ours left to put away it goes back a
     page, which is what B says it does everywhere else on this device and
     what, in a browser, Escape has never done. */
  function back() {
    if (state.hints) return away();
    if (state.card) return shut();
    if (state.found) return unfind();
    if (owns(document.activeElement)) return document.activeElement.blur();
    if (state.standing) return unstand();
    history.back();
  }

  function typingNow() {
    const el = document.activeElement;
    if (!el) return false;
    if (state.host && el === state.host) return false;
    return owns(el);
  }

  function swallow(event) {
    event.preventDefault();
    event.stopPropagation();
  }

  /* ----------------------------------------------------------------- events */

  addEventListener(
    'keydown',
    (event) => {
      if (event.altKey || event.ctrlKey || event.metaKey) return;
      if (event.key === 'Escape') {
        /* Escape is the browser's own way out of a video that has the whole
           screen, and taking that would be taking the way back from somebody
           who is already looking at a page with nothing else on it. */
        if (document.fullscreenElement) return;
        swallow(event);
        back();
        return;
      }
      if (event.key === 'Enter' && state.card) {
        swallow(event);
        chose();
        return;
      }
      const way = OF[event.key];
      if (!way) return;
      if (state.hints) {
        swallow(event);
        typed(way);
        return;
      }
      if (state.card) {
        /* Up and down are the rows; left and right stay the caret's, because
           the line above them is being typed into. */
        if (way === 'u' || way === 'd') {
          swallow(event);
          step(way === 'd' ? 1 : -1);
        }
        return;
      }
      if (state.found) {
        if (way === 'u' || way === 'd') {
          swallow(event);
          next(way === 'd' ? 1 : -1);
        }
        return;
      }
      if (typingNow()) return;
      swallow(event);
      walk(way);
    },
    true,
  );

  /* Y. A finger holding a row down means the browser's own menu and gets it:
     the pad is a mouse and a finger is a touch, and the two are told apart by
     what the event says about itself rather than by a mode. */
  addEventListener(
    'contextmenu',
    (event) => {
      if (event.mozInputSource === 5) return;
      swallow(event);
      if (state.card) return;
      if (state.hints) return again();
      hints('take');
    },
    true,
  );

  /* A. What is stood on, and only if nothing is: a click that lands on our own
     card is that card's own business, and a page with no highlight is a page
     where A is what it has always been, which is a click. */
  addEventListener(
    'click',
    (event) => {
      const path = event.composedPath ? event.composedPath() : [];
      if (state.host && path.includes(state.host)) return;
      if (state.card) {
        swallow(event);
        chose();
        return;
      }
      if (state.hints) {
        swallow(event);
        return;
      }
      if (state.standing) {
        const el = state.standing;
        swallow(event);
        unstand();
        take(el, 'take');
      }
    },
    true,
  );

  /* The pointer, moved, is a hand that has chosen the pointer. Only a real
     push of the stick, because a page that scrolls under a still pointer
     sends one of these too. */
  addEventListener(
    'mousemove',
    (event) => {
      if (!state.standing) {
        state.pointer = { x: event.clientX, y: event.clientY };
        return;
      }
      if (!state.pointer) {
        state.pointer = { x: event.clientX, y: event.clientY };
        return;
      }
      const far = Math.abs(event.clientX - state.pointer.x) + Math.abs(event.clientY - state.pointer.y);
      if (far > DRIFT) unstand();
    },
    true,
  );

  /* What is drawn over the page is drawn where the page was. Moved, all of it
     is in the wrong place, and a label in the wrong place is worse than none:
     it is an answer to a question nobody asked. */
  let owed = false;
  function moved() {
    if (owed) return;
    owed = true;
    requestAnimationFrame(() => {
      owed = false;
      if (state.hints) {
        for (const mark of state.hints.marks) {
          if (!mark.target) continue;
          const rect = seen(mark.target);
          mark.node.classList.toggle('gone', !rect);
          if (rect) place(mark.node, rect);
        }
      }
      if (state.standing) {
        const box = state.root && state.root.querySelector('.standing');
        const rect = seen(state.standing);
        if (!box) return;
        if (!rect) return unstand();
        box.style.left = `${rect.left}px`;
        box.style.top = `${rect.top}px`;
        box.style.width = `${rect.width}px`;
        box.style.height = `${rect.height}px`;
      }
    });
  }
  addEventListener('scroll', moved, true);
  addEventListener('resize', moved, true);

  function ask(said) {
    return browser.runtime.sendMessage(said).catch(() => null);
  }

  /* The new tab is a page of ours, so the card can simply be opened on it.
     Everywhere else it is opened by a press. */
  if (document.documentElement.dataset.console === 'new') {
    if (document.readyState === 'loading') addEventListener('DOMContentLoaded', () => raise(searching()));
    else raise(searching());
  }
})();
