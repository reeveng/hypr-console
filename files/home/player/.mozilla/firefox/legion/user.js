// The browser, told the things a stylesheet cannot tell it.
//
// The colours between the markers are written by tools/legion-theme out of
// theme/palette.toml. Everything else here is Firefox being asked to hold
// still: this device does not animate, and a browser that fades and slides is
// the one window on it that would.

user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);
user_pref("browser.theme.dark-private-windows", true);
user_pref("layout.css.prefers-color-scheme.content-override", 0);
user_pref("ui.systemUsesDarkTheme", 1);

user_pref("ui.prefersReducedMotion", 1);
user_pref("toolkit.cosmeticAnimations.enabled", false);
user_pref("browser.tabs.animate", false);
user_pref("browser.fullscreen.animate", false);

// Held with two hands at arm's length, so the smallest thing on a page is a
// little larger than a desk would want it.
user_pref("browser.display.use_document_fonts", 1);
user_pref("font.minimum-size.x-western", 14);

// legion-theme:begin
user_pref("browser.display.background_color", "#110b12");
user_pref("browser.display.background_color.dark", "#110b12");
user_pref("browser.display.foreground_color", "#ebdce7");
user_pref("browser.anchor_color", "#9dd8ff");
user_pref("browser.visited_color", "#dbc2ff");
user_pref("browser.active_color", "#ffb5e2");
// legion-theme:end
