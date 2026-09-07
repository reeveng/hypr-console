// UI test for EXPLICIT022 — a check waits for the thing, not for a number of
// seconds on the handheld.

// Stands in for `console_test_stages::device::Device`, which a ui case has
// nothing to link against. The rule reads the tail of the resolved path, so
// this resolves the same way the real one does.
mod device {
    pub struct Device;

    pub enum Waited {
        Happened,
        RanOut,
    }

    impl Device {
        pub fn settle(&mut self, _seconds: f64) {}

        pub fn until(&mut self, mut what: impl FnMut(&mut Self) -> bool, seconds: f64) -> Waited {
            let mut left = seconds;

            while left > 0.0 {
                // GOOD — this is the gap between two questions, and the
                // question is asked on the line after it.
                #[cfg_attr(
                    dylint_lib = "explicit022_no_settling",
                    allow(
                        explicit022_no_settling,
                        reason = "this is the gap `until` puts between two questions to the handheld, which is the one caller the sleep's own reason is true of"
                    )
                )]
                self.settle(0.5);

                if what(self) {
                    return Waited::Happened;
                }

                left -= 0.5;
            }

            Waited::RanOut
        }

        pub fn drawn(&mut self, seconds: f64) -> Waited {
            self.until(|_| true, seconds)
        }

        pub fn menus(&mut self) -> Vec<String> {
            Vec::new()
        }
    }
}

use device::Device;

// BAD EXPLICIT022 — a guess about how long the menu takes to come up.
fn waits_a_number(stage: &mut Device) -> bool {
    //~v EXPLICIT022_NO_SETTLING
    stage.settle(1.4);

    !stage.menus().is_empty()
}

// GOOD — the same wait, said as the thing being waited for.
fn waits_for_the_menu(stage: &mut Device) -> bool {
    match stage.drawn(4.0) {
        device::Waited::Happened => true,
        device::Waited::RanOut => false,
    }
}

// GOOD — a `settle` that is not this one. Four other types in the workspace
// spell a word this way and none of them is a clock.
struct Boost;

impl Boost {
    fn settle(&mut self, _seconds: f64) {}
}

fn a_different_settling(boost: &mut Boost) {
    boost.settle(1.0);
}

fn main() {}
