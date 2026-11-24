//! A client's registry queue and the standard globals used by multiple clients.

use std::ops::RangeInclusive;

use crate::client::globals::{BindError, GlobalError, GlobalList, GlobalListContents};
use crate::client::protocol::{wl_compositor, wl_output, wl_registry, wl_seat, wl_shm};
use crate::client::{Connection, Dispatch, EventQueue, QueueHandle};

pub fn queue<State>(
  connection: &Connection,
) -> Result<(GlobalList, EventQueue<State>), GlobalError>
where
  State: Dispatch<wl_registry::WlRegistry, GlobalListContents> + 'static,
{
  crate::client::globals::registry_queue_init(connection)
}

pub fn compositor<State>(
  globals: &GlobalList,
  hand: &QueueHandle<State>,
) -> Result<wl_compositor::WlCompositor, BindError>
where
  State: Dispatch<wl_compositor::WlCompositor, ()> + 'static,
{
  globals.bind(hand, 1..=6, ())
}

pub fn shared_memory<State>(
  globals: &GlobalList,
  hand: &QueueHandle<State>,
) -> Result<wl_shm::WlShm, BindError>
where
  State: Dispatch<wl_shm::WlShm, ()> + 'static,
{
  globals.bind(hand, 1..=1, ())
}

pub fn seat<State>(
  globals: &GlobalList,
  hand: &QueueHandle<State>,
  versions: RangeInclusive<u32>,
) -> Result<wl_seat::WlSeat, BindError>
where
  State: Dispatch<wl_seat::WlSeat, ()> + 'static,
{
  globals.bind(hand, versions, ())
}

pub fn output<State>(
  globals: &GlobalList,
  hand: &QueueHandle<State>,
  versions: RangeInclusive<u32>,
) -> Result<wl_output::WlOutput, BindError>
where
  State: Dispatch<wl_output::WlOutput, ()> + 'static,
{
  globals.bind(hand, versions, ())
}
