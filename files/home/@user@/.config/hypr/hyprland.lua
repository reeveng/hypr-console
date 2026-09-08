-- The Legion Go desktop.
--
-- The controller is read below the compositor, so every button means the same
-- thing in every program. /etc/inputplumber/profiles/router.yaml sends each
-- button somewhere it can be told from the others, and the controller daemon
-- decides what any of it means. Nothing is bound in this file at all -- not the
-- pad and not the keys either, since the same table now says what both of them
-- do; the last section is the argument for that.
--
-- Reload after an edit with:  hyprctl reload. The binds come back on their
-- own: the daemon hears configreloaded and asks the compositor what it is
-- still holding, because a reload keeps only what a file holds and none of
-- them are in one.

------------------------------------------------------------------ the screen

-- The panel is mounted portrait and rotated a quarter turn into landscape.
hl.monitor({
    output    = "eDP-1",
    mode      = "1600x2560@144",
    position  = "auto",
    scale     = 2.5,
    transform = 1,
})

hl.env("XCURSOR_SIZE", "48")
hl.env("HYPRCURSOR_SIZE", "48")
hl.env("MOZ_ENABLE_WAYLAND", "1")

-- A browser of the Firefox family makes itself a fresh profile per installation
-- and records it in profiles.ini under an [Install<hash>] heading, which then
-- outranks the Default=1 this desktop ships. So the profile named `console` --
-- the one holding the colours, the preferences and this desktop's own add-on --
-- was written, installed into, and never once opened: the browser had quietly
-- made itself another one and was running that. This is the switch that turns
-- the behaviour off, and it is the documented one.
hl.env("MOZ_LEGACY_PROFILES", "1")

-- Qt has no Plasma session here to ask, so without this it chooses a platform
-- theme of its own and never reads ~/.config/kdeglobals, where this palette is
-- written for it. "kde" names plasma-integration, the plugin that does read it.
-- kweather is what notices.
hl.env("QT_QPA_PLATFORMTHEME", "kde")

------------------------------------------------------------------ look

-- The palette, written by tools/console-palette out of theme/palette.toml. Every
-- other surface on this machine imports the palette in its own language; the
-- compositor is handed a copy instead, because a config file that fails to
-- load here does not cost a window, it abandons every line after the failure
-- and leaves a session with no bindings on a device whose only other way in is
-- ssh.
-- console-palette:begin
local blossom = {
    active   = "rgba(ffc2e7ff)",
    inactive = "rgba(8a7d8eff)",
    behind   = "rgba(110b12ff)",
}
-- console-palette:end

-- No gaps and no rounding: on an 8.8 inch screen a window should have the
-- whole of it. With a tiling layout one window is already full screen.
hl.config({
    general = {
        gaps_in     = 0,
        gaps_out    = 0,
        border_size = 3,
        -- The window you are typing into is the pink one.
        ["col.active_border"]   = blossom.active,
        ["col.inactive_border"] = blossom.inactive,
    },
    decoration = {
        rounding = 0,
        -- Blur is Hyprland's default and was never chosen here, which is the
        -- whole reason it lasted: nothing in this file mentioned it, so
        -- nothing in this file argued for it. It is the same bargain as an
        -- animation, and the paragraph below refuses that one in as many
        -- words -- work done every frame for something nobody asked to see.
        --
        -- It buys nothing on this desktop. Blur shows through what is on top
        -- of it, and nothing here is transparent: every surface in the palette
        -- is opaque, the windows are full screen with no gaps, and the bar and
        -- the panels are solid `panel` on solid `night`. There has never been
        -- anything to see through.
        --
        -- It is also the suspect for the workspace switch, which is what sent
        -- somebody looking. Switching workspaces showed both of them for a few
        -- milliseconds, on a desktop with animations off where that should be
        -- one frame to the next. The screen is 144Hz, so "a few milliseconds"
        -- is one frame; blur samples the framebuffer behind a surface, and the
        -- bar and the wallpaper are layer surfaces over the windows, so a
        -- blurred layer sampling the workspace being left is a mechanism that
        -- would look exactly like that. Suspect and not culprit: it has not
        -- been watched with blur off and then on again. If the switch still
        -- shows both, the cause is elsewhere and this is still right.
        blur = {
            enabled = false,
        },
        -- Shadows go for the same reason and one of their own. A shadow is a
        -- dark edge drawn to lift a window off what is behind it, and it is
        -- read by seeing it darker than its surroundings. This desktop is dark
        -- first: `night` is #110b12 and the ground behind every window is that
        -- colour, so a shadow is a dark edge on an almost black field. There
        -- is next to nothing to see, and what little there is, is not worth
        -- drawing every frame.
        --
        -- What actually lifts a window here is the border: three pixels of
        -- `pink` on the one being typed into, which is the same pink a
        -- highlighted row is everywhere else on this machine. That is the
        -- affordance, it is deliberate, and it does not need a shadow's help.
        shadow = {
            enabled = false,
        },
    },
    -- Nothing slides, fades or bounces. A window appears where it is going to
    -- be. On a handheld an animation is time spent between deciding something
    -- and seeing it.
    animations = {
        enabled = false,
    },
    misc = {
        force_default_wallpaper = 0,
        disable_hyprland_logo   = true,
        -- What the screen is where no window and no wallpaper covers it.
        -- Hyprland's own default is a neutral grey, close enough to a
        -- background that the wallpaper daemon stopped working and nobody
        -- went looking. Told the palette, the desktop is the right colour
        -- even with nothing painting on it.
        background_color        = blossom.behind,
    },
    -- Redraw the whole screen whenever any of it changes, rather than the
    -- rectangle that changed.
    --
    -- A menu was found painted on the screen with no menu behind it: no
    -- process, nothing in the compositor's list of layers, nothing in its list
    -- of windows. The pixels were simply still there, minutes after the thing
    -- that drew them had gone, and the bar above them went on ticking over the
    -- top. Somebody holding the device pressed every button on it and the
    -- picture never changed, which is a machine that looks broken; the buttons
    -- were working the whole time, and the panels they opened and closed were
    -- opening and closing under a photograph of one of them.
    --
    -- Drawing only what changed is right when what changed is known. This is
    -- the setting that stops trusting that, and it is worth what it costs: the
    -- screen is small, the desktop repaints when a person does something
    -- rather than sixty times a second, and the flip to the panel happens
    -- either way -- what grows is the area shaded on the way there. Against
    -- that, a screen that can go on showing something that is not there is the
    -- one fault a person cannot work around, because everything they would try
    -- looks like it did nothing.
    debug = {
        damage_tracking = 1,
    },
    input = {
        kb_layout    = "us",
        follow_mouse = 1,
        sensitivity  = 0,
    },
    dwindle = {
        preserve_split = true,
    },
})

-- The touchscreen reports in the panel's own orientation, and the panel is
-- mounted a quarter turn from the way the picture is drawn. Naming the output
-- alone left the transform at 0 while the screen sat at 1, so touches landed
-- rotated. Say both.
hl.config({
    input = {
        touchdevice = {
            output    = "eDP-1",
            transform = 1,
        },
    },
})

hl.device({
    name   = "nvtk0603:00-0603:f001",
    output = "eDP-1",
})

-- The controller's touchpad is read by InputPlumber, which turns it into
-- pointer motion. The compositor sees it too, as an absolute touch device
-- standing in for the whole screen, and acting on both at once would send the
-- cursor to two places for one movement. Only one of them may have it.
hl.device({
    name    = "--legion-controller--touchpad",
    enabled = false,
})

-- One window at a time. On a screen this size a split is two unusable halves,
-- so every window takes the whole of it and you move between them with the
-- shoulders and View rather than looking at two things at once.
hl.window_rule({
    name     = "every window fills the screen",
    match    = { class = ".*" },
    maximize = true,
})

-- Nothing floats. A dialog that asks to be a small window on top of another
-- one is the only way anything could overlap here, and on a screen this size
-- an overlay is a window you cannot reach the rest of. Dialogs join the
-- tiling like everything else and fill the screen in their turn.
--
-- The menu is not a window at all. It is a layer surface, so it sits above
-- this and is unaffected.
hl.window_rule({
    name  = "nothing floats",
    match = { class = ".*" },
    tile  = true,
})

-- One window to a workspace. Only one window at a time can hold the maximised
-- state, so a second window opening on the same workspace splits the screen
-- with the first, which is the thing this desktop is meant not to do. Giving
-- each window a workspace of its own means there is never a second window to
-- share with: the shoulders move between them, and the bar lists them.
hl.window_rule({
    name      = "one window to a workspace",
    match     = { class = ".*" },
    workspace = "emptyn",
})

-- No handler on focus: the dispatcher toggles, so focusing a window that was
-- already filling the screen would shrink it again. Unlike fullscreen, any
-- number of windows can be maximised at once, so the rule alone is enough.

------------------------------------------------------------------ startup

hl.on("hyprland.start", function()
    -- Everything this desktop is made of is a user service, started together
    -- by console.target: the controller daemon, the on-screen keyboard, the
    -- bar, the session restore, and the box that asks for a password.
    --
    -- They used to be launched from here and forgotten. That meant a crash
    -- left a piece silently missing until the next reboot, and what was
    -- running depended on what had been started by hand. Services restart
    -- themselves, say so in the journal, and come up the same every time.
    --
    --   systemctl --user status console-controller
    --   journalctl --user -u console-controller -f
    --   systemctl --user restart console.target
    hl.exec_cmd("/usr/local/bin/session-start")
end)

------------------------------------------------------------------ the binds

-- There are none, and this section is here so that the gap is read as a
-- decision rather than filled by the next person to want a shortcut.
--
-- What a button does is decided in one table in the controller daemon --
-- crates/console-input-controller/src/means.rs -- and a keyboard is in that
-- table now beside the pad. A job carries a list of what it is bound to and
-- each of those says which input it is on, so Super and I and the settings
-- button are two lines of one job rather than a line here and a row over there
-- that have to be remembered to agree. This file used to carry thirty binds
-- under a comment saying they were the doings the pad already names, reached by
-- somebody whose hands are on keys, which was that table's own argument written
-- where the table could not see it.
--
-- The pad is the daemon's to act on and the keys are not. InputPlumber emits a
-- modifier and a key in one frame, so a pad chord matched by the compositor was
-- often acted on alone and landed in whatever window had focus -- that is how X
-- typed a k into a terminal. A keyboard chord has the opposite need and it is
-- the compositor's: a key the daemon merely watched would arrive in the window
-- as well as doing its job, and swallowing one is a thing only whoever owns
-- modifier state can do. So the daemon renders the keyboard half of the table
-- into hyprctl keyword bind and hands it over -- at startup, and again every
-- time ~/.config/console/buttons.toml changes, which is what makes a key moved
-- on the setup screen a key moved before the thumb is off the row.
--
-- Which of the two carries a press out is worked out from the input the binding
-- is on, and nobody writes it down anywhere. That is the whole of why this file
-- has no binds: one written here would be a second place saying what a key
-- means, and the two would agree right up until the day they did not.
--
-- What it costs, said out loud. A hyprctl reload throws the pushed binds away,
-- and so does a compositor that restarted while the daemon did not. The daemon
-- puts them back itself now -- it hears configreloaded on the compositor's own
-- socket, asks what is still held, and sends only what is missing -- which is
-- the whole of why the power key can be a row here rather than a line above.
--
-- This file being lua is what a bind has to be written for, and it took a
-- while to find out. hyprctl keyword is refused by a lua config, in a sentence
-- that exits zero and does not say error, so the binds were rendered right and
-- taken by nobody for as long as this file has existed. They go through
-- hyprctl eval and hl.bind now, and the check asks the compositor what it
-- holds rather than trusting what was sent.
--
-- And a bind is exec_cmd and never a dispatch, so the workspace keys run a
-- hyprctl back at the compositor that just called them -- which buys a
-- keyboard and a pad reading their answer out of the same row, and there is no
-- second vocabulary to keep agreeing with the first.
