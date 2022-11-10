use rocket::serde::Serialize;
use std::f64::consts::PI;

pub const INITIAL_DEVIATION: f64 = 350.0;
pub const MIN_DEVIATION: f64 = 25.0;

#[derive(Copy, Clone, Serialize, Debug, PartialEq)]
pub struct Rating {
    pub value: f64,
    pub deviation: f64,
}

impl Default for Rating {
    fn default() -> Rating {
        Rating {
            value: 1500.0,
            deviation: 350.0,
        }
    }
}

impl PartialOrd for Rating {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl Rating {
    pub fn new(value: f64, deviation: f64) -> Rating {
        Rating { value, deviation }
    }

    pub fn decay_deviation(&mut self, rating_periods: i64, c: f64) {
        for _ in 0..rating_periods {
            self.deviation = (self.deviation * self.deviation + c * c)
                .sqrt()
                .min(INITIAL_DEVIATION);
        }
    }

    pub fn rating_change(self, other: Rating, result: f64) -> f64 {
        let new = self.update(other, result);
        new.value - self.value
    }

    #[must_use]
    pub fn update_with_min_dev(self, other: Rating, result: f64, min_deviation: f64) -> Rating {
        let d_2 = 1.0
            / (Q.powf(2.0)
                * g(other.deviation).powf(2.0)
                * e(self.value, other.value, other.deviation)
                * (1.0 - e(self.value, other.value, other.deviation)));
        let res = Rating {
            value: self.value
                + UPDATE_SPEED
                    * (Q / ((1.0 / self.deviation.powf(2.0)) + (1.0 / (d_2))))
                    * g(other.deviation)
                    * (result - e(self.value, other.value, other.deviation)),
            deviation: (1.0 / (1.0 / self.deviation.powf(2.0) + 1.0 / d_2))
                .sqrt()
                .max(min_deviation),
        };

        if result == 0.0 {
            if res.value >= self.value {
                panic!(
                    "{:#?} lost against {:#?} but rating went up to: {:#?}",
                    self, other, res
                );
            }
        }
        if result == 1.0 {
            if res.value <= self.value {
                panic!(
                    "{:#?} won against {:#?} but rating went down to: {:#?}",
                    self, other, res
                );
            }
        }

        res
    }

    #[must_use]
    pub fn update(self, other: Rating, result: f64) -> Rating {
        Self::update_with_min_dev(self, other, result, MIN_DEVIATION)
    }

    pub fn expected(self, other: Rating) -> f64 {
        1.0 / (1.0
            + 10.0f64.powf(
                //(1.0 - UNCERTAINTY) *
                -g((self.deviation * self.deviation + other.deviation * other.deviation).sqrt())
                    * (self.value - other.value)
                    / 400.0,
            ))
    }
}

const Q: f64 = 0.0057565;
const UNCERTAINTY: f64 = 0.1;
const UPDATE_SPEED: f64 = 1.0;

pub fn g(rd: f64) -> f64 {
    1.0 / (1.0 + 3.0 * Q * Q * rd * rd / (PI * PI)).sqrt()
}

pub fn e(r: f64, r_j: f64, rd_j: f64) -> f64 {
    1.0 / (1.0 + 10.0f64.powf((1.0 - UNCERTAINTY) * -g(rd_j) * (r - r_j) / 400.0))
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn blah() {
        let mut a = Rating::new(1800.0, 100.0);
        let mut b = Rating::default();

        for _ in 0..10_000 {
            let new_a = a.update(b, 1.0);
            let new_b = b.update(a, 0.0);
            a = new_a;
            b = new_b;

            let new_a = a.update(b, 0.0);
            let new_b = b.update(a, 1.0);
            a = new_a;
            b = new_b;
        }

        assert_eq!(a, b);
    }
}
