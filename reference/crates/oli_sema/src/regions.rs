//! Regions for escape analysis (`spec/OLI_MEMORY_V0.md` §1, §5).
//!
//! A value carries the set of regions it may point into. `Static` is the
//! neutral element (it outlives everything) so a value that cannot point
//! anywhere carries just `Static`.

use crate::hir::LocalId;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    Static,
    /// Memory reached through parameter `i` of the current procedure.
    Param(usize),
    /// The current procedure's frame.
    Frame,
    /// Objects allocated from the zone whose handle is this local.
    Zone(LocalId),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Regions(Vec<Region>);

impl Regions {
    pub fn stat() -> Regions {
        Regions(vec![Region::Static])
    }

    pub fn one(r: Region) -> Regions {
        Regions(vec![r])
    }

    pub fn atoms(&self) -> &[Region] {
        &self.0
    }

    /// Union: the value may point into any of these. `Static` is neutral and
    /// is dropped as soon as any other region is present.
    pub fn join(&self, other: &Regions) -> Regions {
        let mut out = self.0.clone();
        for r in &other.0 {
            if !out.contains(r) {
                out.push(*r);
            }
        }
        if out.len() > 1 {
            out.retain(|r| *r != Region::Static);
        }
        Regions(out)
    }

    pub fn join_all<'a>(items: impl Iterator<Item = &'a Regions>) -> Regions {
        let mut out = Regions::stat();
        for r in items {
            out = out.join(r);
        }
        out
    }

    /// Does every region of `self` outlive `target`?
    pub fn outlives(&self, target: Region, encloses: &dyn Fn(LocalId, LocalId) -> bool) -> bool {
        self.0.iter().all(|r| outlives(*r, target, encloses))
    }

    /// The regions that die when the current procedure returns.
    pub fn local_atoms(&self) -> Vec<Region> {
        self.0
            .iter()
            .copied()
            .filter(|r| matches!(r, Region::Frame | Region::Zone(_)))
            .collect()
    }

    pub fn describe(&self, local_name: &dyn Fn(LocalId) -> String) -> String {
        let parts: Vec<String> = self
            .0
            .iter()
            .map(|r| match r {
                Region::Static => "static".to_string(),
                Region::Param(i) => format!("param#{i}"),
                Region::Frame => "frame".to_string(),
                Region::Zone(z) => format!("zone {}", local_name(*z)),
            })
            .collect();
        parts.join("|")
    }
}

/// `outlives(a, b)`: is `a` guaranteed to live at least as long as `b`?
/// `encloses(outer, inner)` answers zone nesting.
pub fn outlives(a: Region, b: Region, encloses: &dyn Fn(LocalId, LocalId) -> bool) -> bool {
    match (a, b) {
        (Region::Static, _) => true,
        (Region::Param(_), Region::Static) => false,
        (Region::Param(_), _) => true,
        (Region::Frame, Region::Frame | Region::Zone(_)) => true,
        (Region::Frame, _) => false,
        (Region::Zone(x), Region::Zone(y)) => x == y || encloses(x, y),
        (Region::Zone(_), _) => false,
    }
}
