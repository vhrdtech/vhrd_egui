//! spacenavd's event protocol on its Unix socket (SPM-1).
//!
//! A client that connects and sends nothing gets the original libspnav protocol: every event is eight native-endian
//! `i32`s. The first is the type: `0` motion, `1` button press, `2` button release. A motion carries the six axes
//! (`x y z rx ry rz`) and the time since the last motion event in milliseconds; a button carries its number.

/// The size of one event on the wire: eight `i32`s.
pub const EVENT_SIZE: usize = 8 * 4;

const MOTION: i32 = 0;
const PRESS: i32 = 1;
const RELEASE: i32 = 2;

/// The six axes of the puck as spacenavd sends them: `[x, y, z, rx, ry, rz]`.
///
/// The values are raw counts, scaled by the sensitivity in `/etc/spnavrc`; a firm push reads a few hundred.
/// Which way is which depends on the device and on `/etc/spnavrc` (`swap-yz`, `axismap*`): [`crate::Mapping`]
/// turns them into what an app wants, and the calibrate step measures it.
pub type Axes = [i32; 6];

/// Names of the raw axes, in [`Axes`] order, for settings and the calibration table.
pub const AXIS_NAMES: [&str; 6] = ["x", "y", "z", "rx", "ry", "rz"];

/// One event from spacenavd.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The puck moved (or came back to rest: all axes zero).
    Motion {
        /// The deflection now.
        axes: Axes,
        /// Milliseconds since the previous motion event, as spacenavd counts them.
        period_ms: i32,
    },
    /// A button went down or up.
    Button {
        /// spacenavd's button number (which physical button is which: the calibrate step finds out).
        number: i32,
        /// `true` on press, `false` on release.
        pressed: bool,
    },
    /// An event type this client does not know; kept so a newer spacenavd is visible in the log, not dropped.
    Other {
        /// The type word.
        kind: i32,
    },
}

impl Event {
    /// Decode one event from its eight words.
    pub fn from_words(w: [i32; 8]) -> Self {
        match w[0] {
            MOTION => Event::Motion {
                axes: [w[1], w[2], w[3], w[4], w[5], w[6]],
                period_ms: w[7],
            },
            PRESS | RELEASE => Event::Button {
                number: w[1],
                pressed: w[0] == PRESS,
            },
            kind => Event::Other { kind },
        }
    }

    /// Decode one event from its bytes on the wire (native endian, as spacenavd writes them).
    pub fn from_bytes(b: &[u8; EVENT_SIZE]) -> Self {
        let mut w = [0i32; 8];
        for (i, word) in w.iter_mut().enumerate() {
            let mut four = [0u8; 4];
            four.copy_from_slice(&b[i * 4..i * 4 + 4]);
            *word = i32::from_ne_bytes(four);
        }
        Self::from_words(w)
    }

    /// The eight words of this event, as spacenavd would send it (for fakes and tests).
    pub fn to_words(&self) -> [i32; 8] {
        match *self {
            Event::Motion { axes, period_ms } => [
                MOTION, axes[0], axes[1], axes[2], axes[3], axes[4], axes[5], period_ms,
            ],
            Event::Button { number, pressed } => [
                if pressed { PRESS } else { RELEASE },
                number,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
            Event::Other { kind } => [kind, 0, 0, 0, 0, 0, 0, 0],
        }
    }

    /// The bytes of this event on the wire.
    pub fn to_bytes(&self) -> [u8; EVENT_SIZE] {
        let mut b = [0u8; EVENT_SIZE];
        for (i, word) in self.to_words().iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&word.to_ne_bytes());
        }
        b
    }

    /// A motion event with these axes (period 0), for fakes and tests.
    pub fn motion(axes: Axes) -> Self {
        Event::Motion { axes, period_ms: 0 }
    }

    /// A button press or release, for fakes and tests.
    pub fn button(number: i32, pressed: bool) -> Self {
        Event::Button { number, pressed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_three_types() {
        assert_eq!(
            Event::from_words([0, 1, -2, 3, -4, 5, -6, 16]),
            Event::Motion {
                axes: [1, -2, 3, -4, 5, -6],
                period_ms: 16
            }
        );
        assert_eq!(
            Event::from_words([1, 7, 0, 0, 0, 0, 0, 0]),
            Event::button(7, true)
        );
        assert_eq!(
            Event::from_words([2, 7, 0, 0, 0, 0, 0, 0]),
            Event::button(7, false)
        );
        assert_eq!(
            Event::from_words([9, 0, 0, 0, 0, 0, 0, 0]),
            Event::Other { kind: 9 }
        );
    }

    #[test]
    fn bytes_round_trip() {
        for e in [
            Event::Motion {
                axes: [350, -350, 0, 1, -1, 120],
                period_ms: 8,
            },
            Event::button(1, true),
            Event::button(0, false),
        ] {
            assert_eq!(Event::from_bytes(&e.to_bytes()), e);
        }
    }
}
