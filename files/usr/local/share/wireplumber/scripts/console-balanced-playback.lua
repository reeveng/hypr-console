local linking = require("linking-utils")
local log = Log.open_topic("console-balanced-playback")
local settings = Conf.get_section_as_json("console.balanced-playback", Json.Object {}):parse()
local peak = settings["sample-peak"]
local sustained = settings["sustained-level"]
local max_gain = settings["max-gain"] or 1
local noise_floor = settings["noise-floor"] or 0
local full_gain_level = settings["full-gain-level"] or 1
local automatic_setting = "console.balanced-playback.automatic-leveling"
-- WirePlumber's configured filter rules allocate their indices from zero.
local stream_graph_param = "audioconvert.filter-graph.100"
local mix_headroom = 10
local lookahead = 0.005
local filters = {}
local streams = {}
local stream_graphs = {}
local generation = 0
local app_state = State("console-balanced-playback-apps")
local bypassed_apps = app_state:load()
-- Keep the exported proxy alive for the script's lifetime.
app_metadata = ImplMetadata("console-balanced-playback")

local function app_key(properties)
  for _, field in ipairs { "application.id", "application.name" } do
    local value = properties[field]
    if value and #value > 0 and #value <= 512 then
      local hex = value:gsub(".", function(byte) return string.format("%02x", byte:byte()) end)
      return "app." .. field .. "." .. hex
    end
  end
end

local function stage(name, label, control, config)
  return Json.Object {
    type = "builtin", name = name, label = label, control = control, config = config,
  }
end

local function link(output, input)
  return Json.Object { output = output, input = input }
end

local function output_positions(properties)
  local positions = {}
  for position in (properties["audio.position"] or ""):gmatch("[%w_]+") do
    positions[#positions + 1] = position
  end
  if #positions > 0 then
    return positions
  end
  if tonumber(properties["audio.channels"]) == 1 then
    return { "MONO" }
  end
  return { "FL", "FR" }
end

local function add_level_history(nodes, links, max_boost_floor)
  -- A three-second moving level per stream. It can only remove a little
  -- boost after an active passage, so silence cannot train a gain increase.
  local coefficients = {}
  for _, rate in ipairs { 8000, 16000, 24000, 32000, 44100, 48000, 88200, 96000, 176400, 192000 } do
    local decay = math.exp(-1 / (3 * rate))
    coefficients[#coefficients + 1] = Json.Object {
      rate = rate, b0 = 1 - decay, b1 = 0, b2 = 0, a0 = 1, a1 = -decay, a2 = 0,
    }
  end
  local extra_floor = max_boost_floor * 0.20
  -- A short loud sound should leave the ordinary recovery alone.
  local slope = extra_floor / (2 * sustained)
  nodes[#nodes + 1] = stage("history", "bq_raw", nil, Json.Object {
    coefficients = Json.Array(coefficients),
  })
  nodes[#nodes + 1] = stage("history-floor", "linear", Json.Object {
    Mult = slope, Add = max_boost_floor - slope * peak,
  })
  nodes[#nodes + 1] = stage("history-bound", "clamp", Json.Object {
    Min = max_boost_floor, Max = max_boost_floor + extra_floor,
  })
  links[#links + 1] = link("level:Out", "history:In")
  links[#links + 1] = link("history:Out", "history-floor:In")
  links[#links + 1] = link("history-floor:Out", "history-bound:In")
  links[#links + 1] = link("history-bound:Out", "limit:In 4")
end

local function graph(channels, leveling)
  -- The combined output catches short bursts even when the app bypasses
  -- leveling. Leave 3 dB at the sample ceiling for downstream interpolation.
  local ceiling = leveling and peak or peak / math.sqrt(2)
  local target = leveling and sustained or sustained * 1.5
  local attack = leveling and 55 or 150
  local boost = leveling and max_gain or 1
  local delay = Json.Object { ["Delay (s)"] = lookahead }
  local delay_config = Json.Object { ["max-delay"] = lookahead, latency = lookahead }
  local bounds = Json.Object { Min = -mix_headroom, Max = mix_headroom }
  local nodes = {
    stage("square", "mult"),
    stage("attack", "bq_lowpass", Json.Object { Freq = attack, Q = 0.5 }),
    stage("release", "bq_lowpass", Json.Object { Freq = 13, Q = 0.5 }),
    stage("power", "max"),
    stage("positive", "clamp", Json.Object { Min = 0, Max = mix_headroom ^ 2 }),
    stage("level", "sqrt"),
    stage("sustained-limit", "linear", Json.Object { Mult = ceiling / target }),
    stage("peak-delay", "delay", delay, delay_config),
    stage("limit", "max"),
    stage("reciprocal", "recip"),
    stage("gain", "linear", Json.Object { Mult = ceiling }),
  }
  local links = {
    link("square:Out", "attack:In"),
    link("attack:Out", "release:In"),
    link("attack:Out", "power:In 1"),
    link("release:Out", "power:In 2"),
    link("power:Out", "positive:In"),
    link("positive:Out", "level:In"),
    link("level:Out", "sustained-limit:In"),
    link("sustained-limit:Out", "limit:In 1"),
    link("peak-delay:Out", "limit:In 2"),
    link("limit:Out", "reciprocal:In"),
    link("reciprocal:Out", "gain:In"),
  }
  if boost > 1 then
    local max_boost_floor = ceiling / boost
    local slope = (ceiling - max_boost_floor) / (full_gain_level - noise_floor)
    -- PipeWire 1.6 limits each linear multiplier to +/-10.
    local meter_scale = Json.Object { Mult = 10 }
    nodes[#nodes + 1] = stage("boost-scale-first", "linear", meter_scale)
    nodes[#nodes + 1] = stage("boost-scale-second", "linear", meter_scale)
    nodes[#nodes + 1] = stage("boost-floor", "linear", Json.Object {
      Mult = -slope / 100, Add = ceiling + slope * noise_floor,
    })
    nodes[#nodes + 1] = stage("gain-floor", "clamp", Json.Object { Min = max_boost_floor, Max = ceiling })
    links[#links + 1] = link("level:Out", "boost-scale-first:In")
    links[#links + 1] = link("boost-scale-first:Out", "boost-scale-second:In")
    links[#links + 1] = link("boost-scale-second:Out", "boost-floor:In")
    links[#links + 1] = link("boost-floor:Out", "gain-floor:In")
    add_level_history(nodes, links, max_boost_floor)
  else
    nodes[#nodes + 1] = stage("gain-floor", "linear", Json.Object { Mult = 0, Add = ceiling })
    links[#links + 1] = link("level:Out", "gain-floor:In")
  end
  links[#links + 1] = link("gain-floor:Out", "limit:In 3")
  local inputs, outputs = {}, {}
  local maximum, port = nil, 9
  for channel = 1, channels do
    local prefix = "channel-" .. channel
    local sample, magnitude = prefix .. "-sample", prefix .. "-magnitude"
    local delayed, output = prefix .. "-delay", prefix .. "-output"
    nodes[#nodes + 1] = stage(sample, "clamp", bounds)
    nodes[#nodes + 1] = stage(magnitude, "abs")
    nodes[#nodes + 1] = stage(delayed, "delay", delay, delay_config)
    nodes[#nodes + 1] = stage(output, "mult")
    links[#links + 1] = link(sample .. ":Out", magnitude .. ":In")
    links[#links + 1] = link(sample .. ":Out", delayed .. ":In")
    links[#links + 1] = link(delayed .. ":Out", output .. ":In 1")
    links[#links + 1] = link("gain:Out", output .. ":In 2")

    -- A max stage has eight ports. Carry its result into the next if needed.
    if port > 8 then
      local next_maximum = "maximum-" .. channel
      nodes[#nodes + 1] = stage(next_maximum, "max")
      port = 1
      if maximum then
        links[#links + 1] = link(maximum .. ":Out", next_maximum .. ":In 1")
        port = 2
      end
      maximum = next_maximum
    end
    links[#links + 1] = link(magnitude .. ":Out", maximum .. ":In " .. port)
    port = port + 1
    inputs[channel], outputs[channel] = sample .. ":In", output .. ":Out"
  end

  -- Look ahead for the smoothly varying gain. The delayed peak also catches
  -- individual samples that rise faster than the power detector.
  links[#links + 1] = link(maximum .. ":Out", "square:In 1")
  links[#links + 1] = link(maximum .. ":Out", "square:In 2")
  links[#links + 1] = link(maximum .. ":Out", "peak-delay:In")
  return Json.Object {
    nodes = Json.Array(nodes), links = Json.Array(links),
    inputs = Json.Array(inputs), outputs = Json.Array(outputs),
  }
end

local function playback_channels(node)
  for _, name in ipairs { "Format", "EnumFormat" } do
    for pod in node:iterate_params(name) do
      local format = pod:parse()
      local properties = format and format.properties
      if properties and properties.mediaSubtype == "raw" then
        local channels = properties.channels
        if type(channels) == "table" then
          channels = channels[1]
        end
        if type(channels) == "number" and channels >= 1 and channels <= 64 then
          return channels
        end
      end
    end
  end
end

local function indexed_graph(channels, gain)
  local nodes, links, inputs, outputs = {}, {}, {}, {}
  local first = math.min(gain, 10)
  for channel = 1, channels do
    local input, output = "indexed-in-" .. channel, "indexed-out-" .. channel
    nodes[#nodes + 1] = stage(input, "linear", Json.Object { Mult = first })
    nodes[#nodes + 1] = stage(output, "linear", Json.Object { Mult = gain / first })
    links[#links + 1] = link(input .. ":Out", output .. ":In")
    inputs[channel], outputs[channel] = input .. ":In", output .. ":Out"
  end
  return Json.Object {
    nodes = Json.Array(nodes), links = Json.Array(links),
    inputs = Json.Array(inputs), outputs = Json.Array(outputs),
  }
end

local function update_stream(entry)
  local enabled = Settings.get_boolean(automatic_setting) and bypassed_apps[entry.app] ~= "false"
  local channels = enabled and playback_channels(entry.node) or nil
  if entry.channels == channels then
    return
  end
  local description = Pod.None()
  if channels then
    if entry.indexed_gain then
      description = indexed_graph(channels, entry.indexed_gain):get_data()
    else
      if not stream_graphs[channels] then
        stream_graphs[channels] = graph(channels, true):get_data()
      end
      description = stream_graphs[channels]
    end
  end
  -- The stream's channel mixer applies its volume after this graph.
  entry.node:set_param("Props", Pod.Object {
    "Spa:Pod:Object:Param:Props", "Props",
    params = Pod.Struct { stream_graph_param, description },
  })
  entry.channels = channels
end

app_metadata:activate(Features.ALL, function(metadata, fault)
  if fault then
    log:warning("could not expose per-app leveling settings: " .. tostring(fault))
    return
  end
  for key, value in pairs(bypassed_apps) do
    metadata:set(0, key, "Spa:String:JSON", value)
  end
  metadata:connect("changed", function(_metadata, subject, key, _type, value)
    if subject ~= 0 or not key or #key > 1050
        or not key:match("^app%.application%.[a-z]+%.[0-9a-f]+$") then
      return
    end
    bypassed_apps[key] = value == "false" and "false" or nil
    app_state:save_after_timeout(bypassed_apps)
    for _, entry in pairs(streams) do
      if entry.app == key then
        update_stream(entry)
      end
    end
  end)
end)

Settings.subscribe(automatic_setting, function()
  for _, entry in pairs(streams) do
    update_stream(entry)
  end
end)

local function is_output(properties)
  return properties["media.class"] == "Audio/Sink"
      and properties["wireplumber.is-fallback"] ~= "true"
      and properties["console.balance.sink"] == nil
      and properties["node.virtual"] ~= "true"
end

local function filter_name(serial)
  return "console.balance." .. serial
end

local function load_filter(entry)
  local serial = entry.serial
  local name = filter_name(serial)
  local positions = output_positions(entry.node.properties)
  generation = generation + 1
  entry.generation = tostring(generation)
  local args = Json.Object {
    ["node.name"] = name,
    ["node.description"] = "Balanced Playback",
    ["audio.position"] = Json.Array(positions),
    ["filter.graph"] = graph(#positions, false),
    ["capture.props"] = Json.Object {
      ["node.name"] = name,
      ["media.class"] = "Audio/Sink",
      ["node.virtual"] = true,
      ["priority.session"] = 0,
      ["filter.smart"] = true,
      ["filter.smart.name"] = name,
      ["filter.smart.target"] = Json.Object { ["object.serial"] = serial },
      ["console.balance.sink"] = serial,
      ["console.balance.generation"] = entry.generation,
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
      ["console.balance.generation"] = entry.generation,
    },
  }
  entry.module = LocalModule("libpipewire-module-filter-chain", args:get_data(), {})
end

local function retry_filter(entry)
  entry.retry = Core.timeout_add(entry.retry_delay, function()
    entry.retry = nil
    entry.retry_delay = math.min(entry.retry_delay * 2, 30000)
    load_filter(entry)
    if not entry.module then
      retry_filter(entry)
    end
    return false
  end)
end

local function remove_output(serial)
  local entry = filters[serial]
  if not entry then
    return
  end
  filters[serial] = nil
  if entry.retry then
    entry.retry:destroy()
  end
  if entry.module then
    entry.module:unload()
  end
end

local function recover_filter(serial, removed_generation)
  local entry = filters[serial]
  if entry and entry.module and entry.generation == removed_generation then
    local module = entry.module
    entry.module = nil
    module:unload()
    log:warning(entry.node, "balanced playback filter disappeared; restarting")
    retry_filter(entry)
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
        log:debug(stream_node, "balanced playback is unavailable for this output")
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
    local node = event:get_subject()
    local properties = node.properties
    local parent = properties["console.balance.sink"] or properties["console.balance.output"]
    if parent then
      -- Proxy properties are gone when the removal event is dispatched.
      local added_generation = properties["console.balance.generation"]
      node:connect("pw-proxy-destroyed", function()
        recover_filter(parent, added_generation)
      end)
      return
    end
    if properties["media.class"] == "Stream/Output/Audio"
        and properties["library.name"] == "audioconvert/libspa-audioconvert"
        and properties["console.balance.preview"] ~= "true"
        and properties["media.role"] ~= "DSP" then
      local serial = properties["object.serial"]
      local gain = tonumber(properties["console.balance.indexed-gain"])
      local entry = {
        node = node, app = app_key(properties),
        indexed_gain = gain and gain > 0 and gain <= 24 and gain or nil,
      }
      streams[serial] = entry
      node:connect("pw-proxy-destroyed", function()
        streams[serial] = nil
      end)
      update_stream(entry)
      return
    end
    if not is_output(properties) then
      return
    end
    local serial = properties["object.serial"]
    local entry = { node = node, serial = serial, retry_delay = 1000 }
    filters[serial] = entry
    node:connect("pw-proxy-destroyed", function()
      remove_output(serial)
    end)
    load_filter(entry)
    if not entry.module then
      log:warning(node, "could not start balanced playback; retrying")
      retry_filter(entry)
    end
  end,
}:register()

SimpleEventHook {
  name = "console/balanced-playback-format",
  interests = {
    EventInterest {
      Constraint { "event.type", "=", "node-params-changed" },
      Constraint { "event.subject.param-id", "c", "Format", "EnumFormat" },
    },
  },
  execute = function(event)
    local node = event:get_subject()
    local entry = streams[node.properties["object.serial"]]
    if entry then
      update_stream(entry)
    end
  end,
}:register()
