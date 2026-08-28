-- The Legion Go desktop.
--
-- The controller map lives in /etc/inputplumber/profiles/desktop.yaml, below
-- the compositor, so every button means the same thing in every program. That
-- profile turns the buttons that need a compositor into F13-F20, which nothing
-- else on this system uses. Those are bound at the bottom of this file.
--
-- Reload after an edit with:  hyprctl reload

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

------------------------------------------------------------------ look

-- The palette, written by tools/legion-theme out of theme/palette.toml. Every
-- other surface on this machine imports the palette in its own language; the
-- compositor is handed a copy instead, because a config file that fails to
-- load here does not cost a window, it abandons every line after the failure
-- and leaves a session with no bindings on a device whose only other way in is
-- ssh.
-- legion-theme:begin
local blossom = {
    active   = "rgba(ffb5e2ff)",
    inactive = "rgba(807485ff)",
    behind   = "rgba(110b12ff)",
}
-- legion-theme:end

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
    -- by legion.target: the controller daemon, the on-screen keyboard, the
    -- bar, the session restore, and the box that asks for a password.
    --
    -- They used to be launched from here and forgotten. That meant a crash
    -- left a piece silently missing until the next reboot, and what was
    -- running depended on what had been started by hand. Services restart
    -- themselves, say so in the journal, and come up the same every time.
    --
    --   systemctl --user status legion-controller
    --   journalctl --user -u legion-controller -f
    --   systemctl --user restart legion.target
    hl.exec_cmd("/usr/local/bin/session-start")
end)

------------------------------------------------------------------ controller

-- The controller sends these as Super plus a letter, from the InputPlumber
-- desktop profile. They are ordinary shortcuts, so they work whether they come
-- from the controller or from a real keyboard. Keep the two files in step:
-- desktop.yaml decides which button sends which shortcut, this decides what
-- the shortcut does.
--
--   X, Keyboard button ....... Super K ... show or hide the keyboard
--   Menu, Legion left ........ Super R ... launcher
--   View ..................... Super Tab . next window
--   right paddle ............. Super C ... close the window
--   right stick, left paddle . Super F ... fill the screen
--   Screenshot ............... Super S

------------------------------------------------------------------ keyboard

-- For a real keyboard, and for fixing things over ssh.
local mod = "SUPER"
hl.bind(mod .. " + Q",     hl.dsp.exec_cmd("alacritty"))
hl.bind(mod .. " + R",     hl.dsp.exec_cmd("/usr/local/bin/launcher"))
hl.bind(mod .. " + W",     hl.dsp.window.close())
hl.bind(mod .. " + F",     hl.dsp.window.fullscreen())
hl.bind(mod .. " + K",     hl.dsp.exec_cmd("/usr/local/bin/osk"))
hl.bind("XF86Calculator",  hl.dsp.exec_cmd("/usr/local/bin/osk"))
hl.bind(mod .. " + Tab",   hl.dsp.window.cycle_next())
hl.bind(mod .. " + S",     hl.dsp.exec_cmd("/usr/local/bin/legion-screenshot"))
hl.bind(mod .. " + left",  hl.dsp.focus({ direction = "left" }))
hl.bind(mod .. " + right", hl.dsp.focus({ direction = "right" }))
hl.bind(mod .. " + up",    hl.dsp.focus({ direction = "up" }))
hl.bind(mod .. " + down",  hl.dsp.focus({ direction = "down" }))

hl.bind("XF86AudioRaiseVolume", hl.dsp.exec_cmd("legion-volume up"),   { locked = true, repeating = true })
hl.bind("XF86AudioLowerVolume", hl.dsp.exec_cmd("legion-volume down"), { locked = true, repeating = true })
hl.bind("XF86AudioMute",        hl.dsp.exec_cmd("legion-volume mute"), { locked = true, repeating = true })
