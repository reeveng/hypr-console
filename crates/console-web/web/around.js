'use strict';

/* The browser around the page, which used to be the one thing here that could
 * not be reached.
 *
 * `pad.js` walks a page and `browser.js` asks the browser for what a page is
 * allowed to ask for. Neither of them can put the focus in the address bar or
 * open the browser's own menu: those are chrome, and an add-on written in the
 * ordinary way is not allowed to touch chrome. So the bar along the bottom
 * offered everything except the two things a thumb most wanted, and the answer
 * to both was a finger on a small target at the top of the screen.
 *
 * This is an experiment API, which is the one door out of that. It runs in the
 * parent process with the browser's own privileges rather than an add-on's,
 * and it is allowed here for exactly the reason the add-on is unsigned here:
 * LibreWolf is built without MOZ_REQUIRE_SIGNING, so `EXPERIMENTS_ENABLED`
 * follows a pref this desktop sets in `user.js` rather than being false for
 * good. Release Firefox refuses both, in the same breath, which is why the
 * policy offers this to the browser that can take it and no other.
 *
 * What is in here is deliberately small. Everything that can be done from
 * inside a page is done from inside a page, where a mistake is a broken label
 * rather than a broken browser, and this holds only what nothing else can
 * reach: two places chrome keeps, and the one program that is outside the
 * browser altogether.
 */

/* eslint-disable no-undef */

this.around = class extends ExtensionAPI {
  getAPI() {
    /* The window a press came from is the one in front, and there is only ever
       one on this machine: the desktop gives the browser a workspace and the
       browser draws one window in it. */
    const window = () => Services.wm.getMostRecentWindow('navigator:browser');

    return {
      around: {
        /* The address bar, with what is in it taken, so the on-screen keyboard
           types over the address rather than into the end of it. X raises the
           keyboard, as it does everywhere, and Enter is A. */
        async address() {
          const win = window();
          if (!win || !win.gURLBar) return false;
          win.focus();
          win.gURLBar.select();
          return true;
        },

        /* The browser's own menu, which is where its settings, its zoom and
           its history live. It is a list of rows that the arrows walk and
           Enter takes, so a thumb has it the moment it is open. */
        async menu() {
          const win = window();
          if (!win || !win.PanelUI) return false;
          win.focus();
          win.PanelUI.show();
          return true;
        },

        /* The on-screen keyboard, up. Not a toggle: `keyboard-show` is the
           half of the button that only ever shows, so a card that opens while
           the keyboard is already up leaves it where it is rather than taking
           it away from the person it was drawn for.

           This is the one thing here that is not the browser at all. The
           keyboard is a program of this desktop's, it is raised by a signal,
           and no page and no ordinary add-on may send one -- which is the same
           test the two above pass and the reason all three are in this file
           rather than in `browser.js`.

           Nothing waits on it. A keyboard that failed to come up is a card
           that still draws, still takes a row, and still has the pad: the
           search would be worse for it and is not broken by it. */
        async keyboard() {
          try {
            const { Subprocess } = ChromeUtils.importESModule(
              'resource://gre/modules/Subprocess.sys.mjs',
            );
            await Subprocess.call({ command: '/usr/local/bin/keyboard-show', arguments: [] });
            return true;
          } catch (_) {
            return false;
          }
        },
      },
    };
  }
};
