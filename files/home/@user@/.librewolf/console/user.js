// The browser, told the things a stylesheet cannot tell it.
//
// The colours between the markers are written by tools/console-theme out of
// theme/palette.toml. Most of the rest is the browser being asked to hold
// still: this device does not animate, and a browser that fades and slides is
// the one window on it that would. The last of them is the line that lets this
// desktop put its own add-on in the browser at all.

user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);
user_pref("browser.theme.dark-private-windows", true);
user_pref("layout.css.prefers-color-scheme.content-override", 0);
user_pref("ui.systemUsesDarkTheme", 1);

user_pref("ui.prefersReducedMotion", 1);
user_pref("toolkit.cosmeticAnimations.enabled", false);
user_pref("browser.tabs.animate", false);
user_pref("browser.fullscreen.animate", false);

// The add-on this desktop wrote is not signed and never will be. It is packed
// on the machine out of crates/console-web while `console apply` runs, into
// this profile's own extensions/ directory, so there is no store it came from
// and nobody to have signed it.
//
// Two prefs are needed and neither is enough alone, which was watched on the
// device rather than reasoned about. The first lets an unsigned add-on be
// installed at all: LibreWolf is built without MOZ_REQUIRE_SIGNING, and release
// Firefox is not, which is why ours is in this browser and no other. Note that
// it governs a sideload only -- a policy checks the signature whatever this
// says, and that is why the add-on is not named in one.
user_pref("xpinstall.signatures.required", false);

// The second is what leaves it switched on. A browser installs what it finds in
// extensions/ and then disables it, waiting to be asked about an add-on the
// person did not choose; on this machine she did choose it, by running the
// desktop it belongs to. Without this the add-on arrives and does nothing, and
// the only place that is said is a screen nobody opens.
user_pref("extensions.autoDisableScopes", 0);

// The third is what lets the add-on reach the browser around the page.
//
// The address bar and the browser's own menu are chrome, and an add-on written
// in the ordinary way cannot touch chrome: the bar along the bottom of a page
// offered everything except the two things a thumb most wanted. An experiment
// API is the door out of that, and this is what opens it -- the same property
// of this browser that lets an unsigned add-on run at all, since a build
// without MOZ_REQUIRE_SIGNING is one where this pref is read rather than
// ignored. crates/console-web/web/around.js is everything it is spent on.
//
// It has to be true before the add-on carrying an experiment is installed. A
// browser that reads that manifest with this switched off does not disable the
// experiment, it refuses the whole add-on, and the page loses its labels along
// with everything else. `console apply` writes this file and packs the add-on
// in that order, so one restart has both.
user_pref("extensions.experiments.enabled", true);

// And this is what stops the browser asking about it.
//
// An add-on that claims the home page gets a panel on first start -- "an
// extension changed the page you see when you open the browser" -- which is a
// question worth asking about something arriving from a store, and is not a
// question here: this add-on is the desktop, installed by the same apply that
// wrote this file. A browser does not ask it about add-ons it was distributed
// with, and this is how it is told that this is one of those.
user_pref("extensions.installedDistroAddon.web@console", true);

// The bookmarks toolbar, which this desktop has no use for.
//
// LibreWolf shows it always. It is a row of small targets across the top of
// the screen for a machine with no pointer to spare on them, holding things
// nothing here can add to: bookmarking a page wants the star in the address
// bar, which is the same problem one row down. What is wanted often on this
// device is a question typed into the menu, and that is a button away.
user_pref("browser.toolbars.bookmarks.visibility", "never");

// Never offer to start in troubleshoot mode.
//
// A browser counts the times it was started and not shut down properly, and
// after a few of them it opens asking whether to start again with everything
// switched off. On a desk that question is worth asking. Here it is not: this
// browser is stopped by the desktop rather than by the person using it -- the
// session going down, a check putting the screen back, a mode change -- and
// none of that is a browser that has broken. What arrives instead is a window
// asking a question about add-ons, in front of the page somebody opened, on a
// device whose answer to a dialog is a thumb on a controller.
//
// It is the count that is turned off, not the recovery: a browser that really
// does fall over still comes back with the pages that were open.
user_pref("toolkit.startup.max_resumed_crashes", -1);

// Held with two hands at arm's length, so the smallest thing on a page is a
// little larger than a desk would want it.
user_pref("browser.display.use_document_fonts", 1);
user_pref("font.minimum-size.x-western", 14);

// console-theme:begin
user_pref("browser.display.background_color", "#110b12");
user_pref("browser.display.background_color.dark", "#110b12");
user_pref("browser.display.foreground_color", "#f7e7f3");
user_pref("browser.anchor_color", "#a4dbff");
user_pref("browser.visited_color", "#dfcbff");
user_pref("browser.active_color", "#ffc2e7");
// console-theme:end
