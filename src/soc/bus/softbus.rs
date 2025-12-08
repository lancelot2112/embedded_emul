//! DeviceBus owns the SoC physical memory map, handling device registration, hashed lookups,
//! and prioritised overlays so consumers get deterministic address-to-device resolution
//! without mutating shared state. It mirrors the .NET BasicHashedDeviceBus logic while
//! providing Rust-friendly error handling and concurrency semantics.
use std::{collections::BTreeMap, sync::Arc};

use crate::soc::device::Device;

use super::{
    error::{BusError, BusResult},
    range::BusRange,
};

pub type DeviceRef = Arc<dyn Device>;

///Implement the device bus, owning device registrations and address mappings
pub struct DeviceBus {
    // Linear list of devices, allowing O(1) access by ID
    devices: Vec<DeviceRef>,
    // Mapping physical address ranges to Device IDs
    // Key: Start Address -> (End Address, DeviceId, RemapOffset)
    map: BTreeMap<usize, BusRange>,
    _address_size: usize, // in bits
    pending: Vec<BusRange>,
}

impl DeviceBus {
    pub fn new(address_size: usize) -> Self {
        Self {
            devices: Vec::new(),
            map: BTreeMap::new(),
            _address_size: address_size,
            pending: Vec::new(),
        }
    }

    pub fn map_device(
        &mut self,
        device: impl Device + 'static,
        address: usize,
        priority: u8,
    ) -> BusResult<()> {
        // Insert into 'devices' then update 'map'.
        // To handle priority: If ranges overlap, higher priority overrides /splits lower priority ranges.
        let device_range = device.span();
        self.devices.push(Arc::new(device));
        let device_id = self.devices.len() - 1;
        let range = BusRange {
            bus_start: address,
            bus_end: address + device_range.len(),
            device_offset: device_range.start,
            device_id,
            priority,
        };

        if self.has_blocking_overlap(range.bus_start, range.bus_end, range.priority) {
            self.pending.push(range);
            return Ok(());
        }

        self.insert_range(range)
    }

    pub fn unmap(&mut self, address: usize) -> BusResult<()> {
        let key = self
            .range_key_for_address(address)
            .ok_or(BusError::NotMapped { address })?;
        self.map.remove(&key);
        self.activate_pending()?;
        Ok(())
    }

    /// Returns the physical device reference and cloned range that contains `address`.
    /// Callers that need to construct custom views (TLBs, MMUs, etc) can reuse this to
    /// validate that a downstream redirect stays within a single device span.
    pub fn resolve_device_at(&self, address: usize) -> BusResult<(DeviceRef, BusRange)> {
        let mut range = self
            .range_for_address(address)
            .cloned()
            .ok_or(BusError::InvalidAddress { address })?;
        if address > range.bus_start {
            let delta = address - range.bus_start;
            range.bus_start = address;
            range.device_offset += delta;
        }
        Ok((self.devices[range.device_id].clone(), range))
    }
}

impl DeviceBus {
    fn insert_range(&mut self, range: BusRange) -> BusResult<()> {
        self.clear_overlaps(&range)?;
        self.map.insert(range.bus_start, range);
        Ok(())
    }

    fn activate_pending(&mut self) -> BusResult<()> {
        let mut idx = 0;
        while idx < self.pending.len() {
            let should_try = {
                let candidate = &self.pending[idx];
                !self.has_blocking_overlap(candidate.bus_start, candidate.bus_end, candidate.priority)
            };
            if should_try {
                let candidate = self.pending.remove(idx);
                self.insert_range(candidate)?;
            } else {
                idx += 1;
            }
        }
        Ok(())
    }

    fn has_blocking_overlap(&self, start: usize, end: usize, priority: u8) -> bool {
        for key in self.collect_overlap_keys(start, end) {
            if let Some(existing) = self.map.get(&key) {
                if existing.priority >= priority {
                    return true;
                }
            }
        }
        false
    }

    fn range_for_address(&self, address: usize) -> Option<&BusRange> {
        self.map
            .range(..=address)
            .next_back()
            .and_then(|(_, range)| {
                if address < range.bus_end {
                    Some(range)
                } else {
                    None
                }
            })
    }

    fn range_key_for_address(&self, address: usize) -> Option<usize> {
        self.map
            .range(..=address)
            .next_back()
            .and_then(|(start, range)| {
                if address < range.bus_end {
                    Some(*start)
                } else {
                    None
                }
            })
    }

    fn clear_overlaps(&mut self, range: &BusRange) -> BusResult<()> {
        let keys = self.collect_overlap_keys(range.bus_start, range.bus_end);
        let mut reinserts = Vec::new();

        for key in keys {
            if let Some(existing) = self.map.remove(&key) {
                if existing.bus_end <= range.bus_start || existing.bus_start >= range.bus_end {
                    reinserts.push(existing);
                    continue;
                }

                if existing.priority >= range.priority {
                    reinserts.push(existing);
                    for segment in reinserts {
                        self.map.insert(segment.bus_start, segment);
                    }
                    return Err(BusError::Overlap {
                        address: range.bus_start,
                        details: "higher priority mapping already present".into(),
                    });
                }

                if existing.bus_start < range.bus_start {
                    reinserts.push(self.slice_range(
                        &existing,
                        existing.bus_start,
                        range.bus_start,
                    ));
                }

                if existing.bus_end > range.bus_end {
                    reinserts.push(self.slice_range(&existing, range.bus_end, existing.bus_end));
                }
            }
        }

        for segment in reinserts {
            self.map.insert(segment.bus_start, segment);
        }

        Ok(())
    }

    fn collect_overlap_keys(&self, start: usize, end: usize) -> Vec<usize> {
        let mut keys = Vec::new();
        if let Some((&key, range)) = self.map.range(..=start).next_back() {
            if range.bus_end > start {
                keys.push(key);
            }
        }
        for (&key, _) in self.map.range(start..end) {
            keys.push(key);
        }
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    fn slice_range(&mut self, source: &BusRange, start: usize, end: usize) -> BusRange {
        let mut segment = source.clone();
        segment.bus_start = start;
        segment.bus_end = end;
        segment.device_offset = source.device_offset + (start - source.bus_start);
        segment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{AccessContext, DeviceResult, Endianness};
    use std::ops::Range;

    const DEVICE_PRIORITY: u8 = 0;

    struct ProbeDevice {
        name: String,
        backing: Vec<u8>,
    }

    impl ProbeDevice {
        fn new(name: &str, len: usize) -> Self {
            Self::with_fill(name, len, 0)
        }

        fn with_fill(name: &str, len: usize, fill: u8) -> Self {
            Self {
                name: name.to_string(),
                backing: vec![fill; len],
            }
        }
    }

    impl Device for ProbeDevice {
        fn name(&self) -> &str {
            &self.name
        }

        fn span(&self) -> Range<usize> {
            0..self.backing.len()
        }

        fn endianness(&self) -> Endianness {
            Endianness::Little
        }

        fn read(&self, _offset: usize, _out: &mut [u8], _ctx: AccessContext) -> DeviceResult<()> {
            Ok(())
        }

        fn write(&self, _offset: usize, _data: &[u8], _ctx: AccessContext) -> DeviceResult<()> {
            Ok(())
        }
    }

    #[test]
    fn register_device_and_resolve_returns_expected_mapping() {
        let mut bus = DeviceBus::new(32);
        let probe = ProbeDevice::new("probe", 0x2000);
        bus.map_device(probe, 0x4000, DEVICE_PRIORITY)
            .expect("register device");

        let (dev, range) = bus
            .resolve_device_at(0x4000)
            .expect("resolve mapped address");
        assert_eq!(
            dev.name(),
            "probe",
            "resolved handle should map to registered device"
        );
        assert!(
            range.bus_start == 0x4000 && range.bus_end == 0x6000,
            "resolved range should match registered span"
        );

        let (dev, range) = bus
            .resolve_device_at(0x5000)
            .expect("resolve for verification");
        assert_eq!(
            dev.name(),
            "probe",
            "resolved handle should map to registered device"
        );
        assert!(
            range.bus_start == 0x5000 && range.bus_end == 0x6000,
            "resolved range should match registered span"
        );

        assert!(
            bus.resolve_device_at(0x6000).is_err(),
            "unmapped address should error"
        );
    }

    #[test]
    fn lower_priority_blocked_until_higher_removed() {
        let mut bus = DeviceBus::new(32);
        let high = ProbeDevice::with_fill("hi", 0x100, 0xAA);
        bus.map_device(high, 0x8000, DEVICE_PRIORITY + 5)
            .expect("register high priority device");

        let low = ProbeDevice::with_fill("lo", 0x100, 0x33);
        bus.map_device(low, 0x8000, DEVICE_PRIORITY)
            .expect("mapping succeeds");

        let (dev, _) = bus.resolve_device_at(0x8000).expect("resolve address");
        assert_eq!(
            dev.name(),
            "hi",
            "higher priority device should take precedence"
        );
        bus.unmap(0x8000).expect("remove higher priority range");
        let (dev, _) = bus.resolve_device_at(0x8000).expect("resolve address");
        assert_eq!(
            dev.name(),
            "lo",
            "lower priority device should now be visible"
        );
    }

    #[test]
    fn higher_priority_creates_hole_in_lower_range() {
        let mut bus = DeviceBus::new(32);
        let low = ProbeDevice::with_fill("low", 0x200, 0x11);
        bus.map_device(low, 0x2000, DEVICE_PRIORITY)
            .expect("register low priority range");

        let high = ProbeDevice::with_fill("high", 0x40, 0xEE);
        bus.map_device(high, 0x2060, DEVICE_PRIORITY + 10)
            .expect("register high priority slice");

        let (dev, _) = bus.resolve_device_at(0x2000).expect("resolve low start");
        assert_eq!(
            dev.name(),
            "low",
            "low priority device should be visible before high range"
        );
        let (dev, _) = bus.resolve_device_at(0x2060).expect("resolve high start");
        assert_eq!(
            dev.name(),
            "high",
            "high priority device should be visible in its range"
        );
        let (dev, _) = bus
            .resolve_device_at(0x20A0)
            .expect("resolve low after high range");
        assert_eq!(
            dev.name(),
            "low",
            "low priority device should be visible after high range"
        );
    }
}
