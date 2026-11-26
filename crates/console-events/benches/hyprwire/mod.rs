//! Source used to build an optional reference helper against the installed Hyprwire library.

use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

const SOURCE: &str = r####"#include <hyprwire/hyprwire.hpp>

#include <algorithm>
#include <chrono>
#include <iostream>
#include <stdexcept>
#include <sys/poll.h>
#include <vector>

#include "probe_v1-client.hpp"
#include "probe_v1-server.hpp"

using namespace Hyprutils::Memory;
using Clock = std::chrono::steady_clock;
using Samples = std::vector<Clock::duration>;

constexpr uint32_t Version = 1;

void serve(const char* path) {
    auto socket = Hyprwire::IServerSocket::open(path);
    if (!socket)
        throw std::runtime_error("cannot open Hyprwire server");

    auto implementation = makeShared<CProbeV1Impl>(Version, [](CSharedPointer<Hyprwire::IObject> object) {
        auto manager = makeShared<CProbeManagerV1Object>(std::move(object));
        manager->setPing([manager] { manager->sendPong(); });
        manager->setOnDestroy([manager] { manager->setPing(nullptr); });
    });
    socket->addImplementation(std::move(implementation));
    while (socket->dispatchEvents(true)) {}
}

struct Peer {
    CSharedPointer<Hyprwire::IClientSocket> socket;
    CSharedPointer<CCProbeManagerV1Object> manager;

    explicit Peer(const char* path) : socket(Hyprwire::IClientSocket::open(path)) {
        if (!socket || !socket->waitForHandshake())
            throw std::runtime_error("Hyprwire handshake failed");

        auto implementation = makeShared<CCProbeV1Impl>(Version);
        auto spec = implementation->protocol();
        socket->addImplementation(std::move(implementation));
        auto object = socket->bindProtocol(spec, Version);
        if (!object)
            throw std::runtime_error("Hyprwire bind failed");

        manager = makeShared<CCProbeManagerV1Object>(std::move(object));
    }

    void ping() {
        bool arrived = false;
        manager->setPong([&arrived] { arrived = true; });
        const auto deadline = Clock::now() + std::chrono::seconds(5);
        manager->sendPing();
        while (!arrived) {
            const auto remaining = std::chrono::ceil<std::chrono::milliseconds>(deadline - Clock::now());
            pollfd waiting = {.fd = socket->extractLoopFD(), .events = POLLIN, .revents = 0};
            if (remaining.count() <= 0 || poll(&waiting, 1, static_cast<int>(remaining.count())) <= 0)
                throw std::runtime_error("Hyprwire pong timed out");
            if (!socket->dispatchEvents(false))
                throw std::runtime_error("Hyprwire connection ended");
        }
        manager->setPong(nullptr);
    }
};

void sample(const char* path, unsigned count, std::string_view mode) {
    std::chrono::nanoseconds elapsed(0);
    constexpr unsigned Window = 16384;
    std::vector<int64_t> samples(std::min(count, Window));
    const auto record = [&](unsigned iteration, auto duration) {
        const auto nanos = std::chrono::duration_cast<std::chrono::nanoseconds>(duration);
        samples[iteration % Window] = nanos.count();
        elapsed += nanos;
    };
    if (mode == "warm") {
        Peer peer(path);
        peer.ping();
        for (unsigned iteration = 0; iteration < count; ++iteration) {
            const auto began = Clock::now();
            peer.ping();
            record(iteration, Clock::now() - began);
        }
    } else if (mode == "cold") {
        for (unsigned iteration = 0; iteration < count; ++iteration) {
            const auto began = Clock::now();
            Peer peer(path);
            peer.ping();
            record(iteration, Clock::now() - began);
        }
    } else {
        throw std::runtime_error("unknown timing mode");
    }
    std::cout << elapsed.count();
    const auto first = count >= Window ? count % Window : 0;
    for (unsigned offset = 0; offset < samples.size(); ++offset)
        std::cout << ' ' << samples[(first + offset) % Window];
    std::cout << '\n';
}

int main(int argc, char** argv) {
    try {
        if (argc == 3 && std::string_view(argv[1]) == "serve") {
            serve(argv[2]);
            return 0;
        }
        if (argc == 5 && std::string_view(argv[1]) == "sample") {
            const auto count = std::stoul(argv[2]);
            if (count == 0 || count > 1000000000)
                throw std::runtime_error("sample count must be between 1 and 1000000000");
            sample(argv[3], static_cast<unsigned>(count), argv[4]);
            return 0;
        }
        throw std::runtime_error("usage: hyprwire_benchmark serve SOCKET | sample COUNT SOCKET MODE");
    } catch (const std::exception& fault) {
        std::cerr << fault.what() << '\n';
        return 1;
    }
}
"####;

const SCHEMA: &str = r####"<?xml version="1.0" encoding="UTF-8"?>
<protocol name="probe_v1" version="1">
  <object name="probe_manager_v1" version="1">
    <c2s name="ping"/>
    <s2c name="pong"/>
  </object>
</protocol>
"####;

#[cfg_attr(
  dylint_lib = "explicit036_no_program_by_name",
  expect(
    explicit036_no_program_by_name,
    reason = "these optional host tools are supplied by the benchmark operator and are not Console runtime dependencies"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "the private scratch directory is unique to this benchmark and the scanner is run only after both inputs are written"
  )
)]
pub fn build(at: &Path) -> Result<PathBuf, Box<dyn Error>> {
  let schema = at.join("probe.xml");
  let source = at.join("hyprwire.cpp");
  let binary = at.join("hyprwire");
  let mut schema_file = std::fs::File::create_new(&schema)?;
  schema_file.write_all(SCHEMA.as_bytes())?;
  let mut source_file = std::fs::File::create_new(&source)?;
  source_file.write_all(SOURCE.as_bytes())?;
  let modes: [&[&str]; 2] = [&[], &["--client"]];

  for flags in modes {
    let generated = Command::new("hyprwire-scanner")
      .args(flags)
      .arg(&schema)
      .arg(at)
      .output()?;
    checked(generated)?;
  }

  let pkg = Command::new("pkg-config")
    .args(["--cflags", "--libs", "hyprwire", "hyprutils"])
    .output()?;
  let stdout = checked(pkg)?;
  let flags = String::from_utf8(stdout)?;
  let compiled = Command::new("c++")
    .args(["-std=c++23", "-O3", "-pthread", "-I"])
    .arg(at)
    .arg(source)
    .arg(at.join("probe_v1-server.cpp"))
    .arg(at.join("probe_v1-client.cpp"))
    .args(flags.split_whitespace())
    .arg("-o")
    .arg(&binary)
    .output()?;
  checked(compiled)?;

  Ok(binary)
}

pub fn sample(
  binary: &Path,
  socket: &Path,
  mode: Mode,
  count: u64,
) -> Result<(Duration, Vec<Duration>), Box<dyn Error>> {
  let mode_2 = match mode {
    Mode::Warm => "warm",
    Mode::Cold => "cold",
  };
  let measured = Command::new(binary)
    .args(["sample", &count.to_string()])
    .arg(socket)
    .arg(mode_2)
    .output()?;
  let bytes = checked(measured)?;
  let output = String::from_utf8(bytes)?;
  let mut words = output.split_whitespace();
  let first = words.next().ok_or("Hyprwire sample had no time")?;
  let nanos = first.parse()?;
  let individual = words
    .map(|word| word.parse().map(Duration::from_nanos))
    .collect::<Result<Vec<_>, _>>()?;

  Ok((Duration::from_nanos(nanos), individual))
}

#[derive(Clone, Copy)]
pub enum Mode {
  Cold,
  Warm,
}

fn checked(output: Output) -> Result<Vec<u8>, Box<dyn Error>> {
  match output.status.success().then_some(()) {
    Some(()) => Ok(output.stdout),
    None => {
      let diagnostic = String::from_utf8(output.stderr)?;
      Err(Box::from(diagnostic))
    }
  }
}
