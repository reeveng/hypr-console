local position = ...
local ids = {}
for _, workspace in ipairs(hl.get_workspaces()) do
    if workspace.id > 0 then
        ids[#ids + 1] = workspace.id
    end
end
table.sort(ids)
return ids[position] or math.max(position, (ids[#ids] or 0) + 1)
