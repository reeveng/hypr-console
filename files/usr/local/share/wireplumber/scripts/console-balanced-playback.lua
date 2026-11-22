local linking = require("linking-utils")
local log = Log.open_topic("console-balanced-playback")
local settings = Conf.get_section_as_json("console.balanced-playback", Json.Object {}):parse()
local peak = settings["sample-peak"]
local sustained = settings["sustained-level"]
local mix_headroom = 16
local filters = {}

local function stage(name, label, control)
  return Json.Object { type = "builtin", name = name, label = label, control = control }
end

local function link(output, input)
  return Json.Object { output = output, input = input }
end

local function graph()
  return Json.Object {
    nodes = Json.Array {
      stage("left", "clamp", Json.Object { Min = -mix_headroom, Max = mix_headroom }),
      stage("right", "clamp", Json.Object { Min = -mix_headroom, Max = mix_headroom }),
      stage("left-level", "abs"),
      stage("right-level", "abs"),
      stage("peak", "max"),
      stage("square", "mult"),
      stage("average", "bq_lowpass", Json.Object { Freq = 8, Q = 0.5 }),
      stage("positive", "clamp", Json.Object { Min = 0, Max = mix_headroom ^ 2 }),
      stage("level", "sqrt"),
      stage("sustained-limit", "linear", Json.Object { Mult = peak / sustained }),
      stage("limit", "max"),
      stage("floor", "clamp", Json.Object { Min = peak, Max = mix_headroom * peak / sustained }),
      stage("reciprocal", "recip"),
      stage("gain", "linear", Json.Object { Mult = peak }),
      stage("left-output", "mult"),
      stage("right-output", "mult"),
    },
    links = Json.Array {
      link("left:Out", "left-level:In"),
      link("right:Out", "right-level:In"),
      link("left-level:Out", "peak:In 1"),
      link("right-level:Out", "peak:In 2"),
      link("peak:Out", "square:In 1"),
      link("peak:Out", "square:In 2"),
      link("square:Out", "average:In"),
      link("average:Out", "positive:In"),
      link("positive:Out", "level:In"),
      link("level:Out", "sustained-limit:In"),
      link("sustained-limit:Out", "limit:In 1"),
      link("peak:Out", "limit:In 2"),
      link("limit:Out", "floor:In"),
      link("floor:Out", "reciprocal:In"),
      link("reciprocal:Out", "gain:In"),
      link("gain:Out", "left-output:In 1"),
      link("gain:Out", "right-output:In 1"),
      link("left:Out", "left-output:In 2"),
      link("right:Out", "right-output:In 2"),
    },
    inputs = Json.Array { "left:In", "right:In" },
    outputs = Json.Array { "left-output:Out", "right-output:Out" },
  }
end

local function is_output(properties)
  return properties["media.class"] == "Audio/Sink"
      and properties["wireplumber.is-fallback"] ~= "true"
      and properties["console.balance.sink"] == nil
      and properties["node.virtual"] ~= "true"
end

local function filter_name(serial)
  return "console.balance." .. serial
end

local function start_filter(node)
  local properties = node.properties
  if not is_output(properties) then
    return
  end
  local serial = properties["object.serial"]
  local name = filter_name(serial)
  local args = Json.Object {
    ["node.name"] = name,
    ["node.description"] = "Balanced Playback",
    ["audio.position"] = Json.Array { "FL", "FR" },
    ["filter.graph"] = graph(),
    ["capture.props"] = Json.Object {
      ["node.name"] = name,
      ["media.class"] = "Audio/Sink",
      ["node.virtual"] = true,
      ["priority.session"] = 0,
      ["filter.smart"] = true,
      ["filter.smart.name"] = name,
      ["filter.smart.target"] = Json.Object { ["object.serial"] = serial },
      ["console.balance.sink"] = serial,
    },
    ["playback.props"] = Json.Object {
      ["node.name"] = name .. ".output",
      ["target.object"] = serial,
      ["node.passive"] = true,
      ["node.dont-fallback"] = true,
      ["node.dont-move"] = true,
      ["channelmix.normalize"] = true,
      ["state.restore-props"] = false,
      ["state.default-volume"] = 1.0,
      ["media.role"] = "DSP",
      ["console.balance.output"] = serial,
    },
  }
  local module = LocalModule("libpipewire-module-filter-chain", args:get_data(), {})
  if module then
    filters[node.id] = module
  else
    log:warning(node, "could not start balanced playback")
  end
end

-- Use the output selected by WirePlumber, including an application's explicit
-- choice. The preview and the processor's output leg use that output directly.
SimpleEventHook {
  name = "console/balanced-playback-target",
  after = "linking/get-filter-from-target",
  before = "linking/prepare-link",
  interests = {
    EventInterest { Constraint { "event.type", "=", "select-target" } },
  },
  execute = function(event)
    local _, items, stream, properties, flags, target =
        linking:unwrap_select_target_event(event)
    if not target or properties["media.type"] ~= "Audio" then
      return
    end
    local stream_node = stream:get_associated_proxy("node")
    local target_node = target:get_associated_proxy("node")
    local parent = target_node.properties["console.balance.sink"]
    local direction = properties["item.node.direction"]
    local preview = stream_node.properties["console.balance.preview"] == "true"
    local processor = stream_node.properties["console.balance.output"]
    if parent and (preview or direction == "input") then
      -- A recording of an output should hear the protected mix.
      local output = items:lookup {
        type = "SiLinkable",
        Constraint { "object.serial", "=", parent },
      }
      flags.can_passthrough = false
      event:set_data("target", output)
    elseif direction == "output" and not preview and is_output(target_node.properties) then
      local serial = target_node.properties["object.serial"]
      if processor == serial then
        return
      end
      local filter = items:lookup {
        type = "SiLinkable",
        Constraint { "node.name", "=", filter_name(serial) },
      }
      if filter and linking.canLink(properties, filter)
          and linking.checkPassthroughCompatibility(stream, filter) then
        flags.can_passthrough = false
        event:set_data("target", filter)
      else
        log:warning(stream_node, "balanced playback is unavailable for this output")
        event:set_data("target", nil)
      end
    end
  end,
}:register()

SimpleEventHook {
  name = "console/add-balanced-output",
  interests = {
    EventInterest { Constraint { "event.type", "=", "node-added" } },
  },
  execute = function(event)
    start_filter(event:get_subject())
  end,
}:register()

SimpleEventHook {
  name = "console/remove-balanced-output",
  interests = {
    EventInterest { Constraint { "event.type", "=", "node-removed" } },
  },
  execute = function(event)
    local node = event:get_subject()
    filters[node.id] = nil
  end,
}:register()
