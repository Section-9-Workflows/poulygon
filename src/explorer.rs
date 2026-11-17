// SPDX-License-Identifier: MIT
use crate::scene::{Config, FAMILIES};

pub struct Explorer {
    pub family: usize,
    pub level: usize,
    pub cycle: u64,
    pub safe: [usize; FAMILIES.len()],
    low: usize,
    high: Option<usize>,
    passes: u8,
}
impl Explorer {
    pub fn new() -> Self {
        Self {
            family: 7,
            level: 1,
            cycle: 0,
            safe: [0; FAMILIES.len()],
            low: 0,
            high: None,
            passes: 0,
        }
    }
    pub fn config(&self) -> Config {
        Config::new(self.family, self.level, self.cycle, &self.safe)
    }
    fn next_family(&mut self) {
        self.safe[self.family] = self.low;
        self.family = (self.family + 1) % FAMILIES.len();
        if self.family == 7 {
            self.cycle += 1;
        }
        self.level = 1;
        self.low = 0;
        self.high = None;
        self.passes = 0;
    }
    pub fn result(&mut self, pass: bool) -> Option<&'static str> {
        if !pass {
            self.passes = 0;
            self.high = Some(self.high.map_or(self.level, |h| h.min(self.level)));
            if self.level <= self.low {
                self.low = 0;
                self.safe[self.family] = 0;
            }
            if self.level == 1 {
                self.next_family();
                return Some("baseline_failed");
            }
            self.level = self.low.max(1);
            return Some("back_down");
        }
        self.passes += 1;
        if self.passes < 2 {
            return None;
        }
        self.passes = 0;
        self.low = self.level;
        self.safe[self.family] = self.low;
        if self.level == Config::domain_max(self.family) {
            self.next_family();
            return Some("domain_complete");
        }
        if let Some(high) = self.high {
            if high <= self.low + 1 {
                self.next_family();
                return Some("boundary_found");
            }
            self.level = self.low + (high - self.low) / 2;
            Some("refine")
        } else {
            self.level = self
                .level
                .saturating_mul(2)
                .min(Config::domain_max(self.family));
            Some("increase")
        }
    }
    pub fn summary(&self, last: &str) -> String {
        let mut s=format!("# Poulygon operating envelope\n\n{}\nNext scene: {} at level {}, round {}.\n\n| Workload | Last passing level |\n|---|---:|\n",last,FAMILIES[self.family],self.level,self.cycle);
        for (i, n) in self.safe.iter().enumerate() {
            s.push_str(&format!("| {} | {} |\n", FAMILIES[i], n));
        }
        s.push_str("\nZero means no passing result recorded. Levels have different meanings between scenes; events.jsonl records their actual parameters. These are steady-window observations for this renderer and device, not universal game budgets. Completion wait includes driver and scheduling time. Scene changes and settling are excluded from steady-window statistics.\n");
        s
    }
}
