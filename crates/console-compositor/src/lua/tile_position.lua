function(column, split)
    return function()
        local moving = hl.get_active_window()
        local layout = moving and moving.layout
        if not layout or layout.name ~= "dwindle" or moving.fullscreen ~= 0 or moving.group then
            return
        end

        local target, outer, vertical
        for _, window in ipairs(hl.get_windows({ workspace = moving.workspace, floating = false, mapped = true })) do
            if not window.hidden and window.fullscreen == 0 then
                local at, size = window.at, window.size
                local side = column == "left" and at.x or -(at.x + size.x)
                local end_of_stack = split == "u" and at.y or -(at.y + size.y)
                if not outer or side < outer then
                    outer, target, vertical = side, nil, nil
                end
                if side == outer and window.address ~= moving.address
                    and (not vertical or end_of_stack < vertical) then
                    target, vertical = window, end_of_stack
                end
            end
        end
        if not target then
            return
        end

        hl.dispatch(hl.dsp.focus({ window = target }))
        local focused = hl.get_active_window()
        if not focused or focused.address ~= target.address then
            return
        end

        -- Reinsert beside the focused tile so Dwindle can apply the chosen vertical split.
        hl.dispatch(hl.dsp.window.float({ window = moving, action = "set" }))
        if moving.floating then
            hl.dispatch(hl.dsp.layout("preselect " .. split))
            hl.dispatch(hl.dsp.window.float({ window = moving, action = "unset" }))
            hl.dispatch(hl.dsp.layout("preselect reset"))
        end
        hl.dispatch(hl.dsp.focus({ window = moving }))
    end
end
