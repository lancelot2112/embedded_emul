use std::sync::Arc;

use crate::soc::device::Device;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeKind {
    Device,
    Redirect,
}

#[derive(Debug, Clone)]
pub struct BusRange {
    pub id: u64,
    pub bus_start: u64,
    pub bus_end: u64,
    pub device_offset: u64,
    pub device_id: usize,
    pub priority: u8,
    pub kind: RangeKind,
}

impl BusRange {
    pub fn contains(&self, addr: u64) -> bool {
        self.bus_start <= addr && addr < self.bus_end
    }

    pub fn overlaps(&self, other: &BusRange) -> bool {
        self.bus_start < other.bus_end && other.bus_start < self.bus_end
    }

    pub fn len(&self) -> u64 {
        self.bus_end - self.bus_start
    }

    pub fn is_empty(&self) -> bool {
        self.bus_start == self.bus_end
    }
}

#[derive(Clone)]
pub struct ResolvedRange {
    pub device: Arc<dyn Device>,
    pub bus_start: u64,
    pub bus_end: u64,
    pub device_offset: u64,
    pub priority: u8,
    pub device_id: usize,
}

impl ResolvedRange {
    pub fn len(&self) -> u64 {
        self.bus_end - self.bus_start
    }

    pub fn is_empty(&self) -> bool {
        self.bus_start == self.bus_end
    }

    pub fn contains(&self, addr: u64) -> bool {
        self.bus_start <= addr && addr < self.bus_end
    }
}
