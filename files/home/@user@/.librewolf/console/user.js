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

// Held with two hands at arm's length, so the smallest thing on a page is a
// little larger than a desk would want it.
user_pref("browser.display.use_document_fonts", 1);
user_pref("font.minimum-size.x-western", 14);

// console-theme:begin
user_pref("browser.display.background_color", "#110b12");
user_pref("browser.display.background_color.dark", "#110b12");
user_pref("browser.display.foreground_color", "#ebdce7");
user_pref("browser.anchor_color", "#9dd8ff");
user_pref("browser.visited_color", "#dbc2ff");
user_pref("browser.active_color", "#ffb5e2");
// console-theme:end
