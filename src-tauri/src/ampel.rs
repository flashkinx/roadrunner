//! Minuten -> Ampelfarbe.

use crate::config::Thresholds;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Color {
    Green,
    Yellow,
    Red,
    /// Kein aktueller Wert (Pause, Fehler, nicht eingerichtet).
    Grey,
}

impl Color {
    pub fn rgb(self) -> [u8; 3] {
        // macOS-Systemfarben (hell)
        match self {
            Color::Green => [52, 199, 89],
            Color::Yellow => [255, 204, 0],
            Color::Red => [255, 59, 48],
            Color::Grey => [142, 142, 147],
        }
    }
}

/// Bis `green_max` gruen, bis `yellow_max` gelb, darueber rot. Grenzen inklusive.
pub fn evaluate(value_min: i64, t: &Thresholds) -> Color {
    if value_min <= t.green_max {
        Color::Green
    } else if value_min <= t.yellow_max {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Sekunden auf ganze Minuten runden (kaufmaennisch).
pub fn to_minutes(seconds: u64) -> i64 {
    ((seconds + 30) / 60) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Thresholds = Thresholds { green_max: 45, yellow_max: 55 };

    #[test]
    fn boundaries_inclusive() {
        assert_eq!(evaluate(30, &T), Color::Green);
        assert_eq!(evaluate(45, &T), Color::Green);
        assert_eq!(evaluate(46, &T), Color::Yellow);
        assert_eq!(evaluate(55, &T), Color::Yellow);
        assert_eq!(evaluate(56, &T), Color::Red);
    }

    #[test]
    fn negative_delay_is_green() {
        let d = Thresholds { green_max: 5, yellow_max: 15 };
        assert_eq!(evaluate(-2, &d), Color::Green);
    }

    #[test]
    fn rounding() {
        assert_eq!(to_minutes(2669), 44);
        assert_eq!(to_minutes(2670), 45);
        assert_eq!(to_minutes(0), 0);
    }
}
