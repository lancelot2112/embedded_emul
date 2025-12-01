use std::sync::Arc;

use crate::soc::device::Device;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeKind {
    Device,
    Redirect,
}

#[derive(Debug, Clone)]
pub struct BusRange {
    pub bus_start: usize,
    pub bus_end: usize,
    pub device_offset: usize,
    pub device_id: usize,
    pub priority: u8,
    pub kind: RangeKind,
}

impl BusRange {
    pub fn contains(&self, addr: usize) -> bool {
        self.bus_start <= addr && addr < self.bus_end
    }

    pub fn overlaps(&self, other: &BusRange) -> bool {
        self.bus_start < other.bus_end && other.bus_start < self.bus_end
    }

    pub fn len(&self) -> usize {
        self.bus_end - self.bus_start
    }

    pub fn is_empty(&self) -> bool {
        self.bus_start == self.bus_end
    }
}

#[derive(Clone)]
pub struct ResolvedRange {
    pub device: Arc<dyn Device>,
    pub bus_start: usize,
    pub bus_end: usize,
    pub device_offset: usize,
    pub priority: u8,
    pub device_id: usize,
}

impl ResolvedRange {
    pub fn len(&self) -> usize {
        self.bus_end - self.bus_start
    }

    pub fn is_empty(&self) -> bool {
        self.bus_start == self.bus_end
    }

    pub fn contains(&self, addr: usize) -> bool {
        self.bus_start <= addr && addr < self.bus_end
    }
}
